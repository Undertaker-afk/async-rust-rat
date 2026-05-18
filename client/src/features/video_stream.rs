use std::sync::Arc;
use std::str::FromStr;
use windows::{
    core::*,
    Win32::Media::MediaFoundation::*,
    Win32::Graphics::Direct3D11::*,
    Win32::Graphics::Dxgi::*,
    Win32::Foundation::*,
    Win32::System::Com::*,
    Win32::Graphics::Direct3D::*,
};
use iroh::{Endpoint, RelayMode, RelayMap};
use moq_transport::setup;
use tokio::sync::mpsc;
use crate::features::remote_desktop::capture_screen;
use nostr_sdk::prelude::*;
use zstd::stream::encode_all;
use chacha20poly1305::{aead::{Aead, NewAead}, XChaCha20Poly1305};
use rand::RngCore;

pub struct VideoEncoder {
    transform: IMFTransform,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;

            let mut transform: Option<IMFTransform> = None;
            CoCreateInstance(&CLSID_CMSH264EncoderMFT, None, CLSCTX_ALL, &mut transform)?;
            let transform = transform.ok_or(Error::from_win32(WIN32_ERROR(0)))?;

            let mut output_type: Option<IMFMediaType> = None;
            MFCreateMediaType(&mut output_type)?;
            let output_type = output_type.ok_or(Error::from_win32(WIN32_ERROR(0)))?;
            output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            output_type.SetUINT32(&MF_MT_AVG_BITRATE, 1000000)?;
            MFSetAttributeSize(&output_type, &MF_MT_FRAME_SIZE, width, height)?;
            MFSetAttributeRatio(&output_type, &MF_MT_FRAME_RATE, fps, 1)?;
            output_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;

            transform.SetOutputType(0, &output_type, 0)?;

            let mut input_type: Option<IMFMediaType> = None;
            MFCreateMediaType(&mut input_type)?;
            let input_type = input_type.ok_or(Error::from_win32(WIN32_ERROR(0)))?;
            input_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            input_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            MFSetAttributeSize(&input_type, &MF_MT_FRAME_SIZE, width, height)?;
            MFSetAttributeRatio(&input_type, &MF_MT_FRAME_RATE, fps, 1)?;

            transform.SetInputType(0, &input_type, 0)?;

            transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;

            Ok(Self { transform })
        }
    }

    pub fn encode_frame(&mut self, frame: &[u8], timestamp: i64) -> Result<Vec<Vec<u8>>> {
        unsafe {
            let mut sample: Option<IMFSample> = None;
            MFCreateSample(&mut sample)?;
            let sample = sample.ok_or(Error::from_win32(WIN32_ERROR(0)))?;

            let mut buffer: Option<IMFMediaBuffer> = None;
            MFCreateMemoryBuffer(frame.len() as u32, &mut buffer)?;
            let buffer = buffer.ok_or(Error::from_win32(WIN32_ERROR(0)))?;

            let mut p_data = std::ptr::null_mut();
            buffer.Lock(&mut p_data, None, None)?;
            std::ptr::copy_nonoverlapping(frame.as_ptr(), p_data, frame.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(frame.len() as u32)?;

            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(timestamp)?;

            self.transform.ProcessInput(0, &sample, 0)?;

            let mut outputs = Vec::new();
            loop {
                let mut output_buffer: MFT_OUTPUT_DATA_BUFFER = std::mem::zeroed();
                let mut status = 0;

                let res = self.transform.ProcessOutput(0, std::slice::from_mut(&mut output_buffer), &mut status);

                if res.is_err() {
                    break;
                }

                if let Some(out_sample) = output_buffer.pSample.as_ref() {
                    let mut out_buffer: Option<IMFMediaBuffer> = None;
                    out_sample.ConvertToContiguousBuffer(&mut out_buffer)?;
                    if let Some(out_buffer) = out_buffer {
                        let mut p_out_data = std::ptr::null_mut();
                        let mut length = 0;
                        out_buffer.Lock(&mut p_out_data, None, Some(&mut length))?;
                        let data = std::slice::from_raw_parts(p_out_data, length as usize).to_vec();
                        out_buffer.Unlock()?;
                        outputs.push(data);
                    }
                }
            }
            Ok(outputs)
        }
    }
}

pub struct FallbackEncoder {
    encoder: openh264::encoder::Encoder,
    width: u32,
    height: u32,
}

impl FallbackEncoder {
    pub fn new(width: u32, height: u32) -> Self {
        let config = openh264::encoder::EncoderConfig::new(width, height);
        let encoder = openh264::encoder::Encoder::with_config(config).unwrap();
        Self { encoder, width, height }
    }

    pub fn encode(&mut self, bgr_frame: &[u8]) -> Vec<u8> {
        let yuv = bgr_to_yuv420p(bgr_frame, self.width as usize, self.height as usize);

        let y_stride = self.width as usize;
        let u_stride = self.width as usize / 2;
        let v_stride = self.width as usize / 2;

        let y_len = y_stride * self.height as usize;
        let u_len = u_stride * (self.height as usize / 2);

        let y = &yuv[0..y_len];
        let u = &yuv[y_len..y_len + u_len];
        let v = &yuv[y_len + u_len..];

        let source = openh264::formats::YUVSource::with_strides(
            self.width as usize,
            self.height as usize,
            y, y_stride,
            u, u_stride,
            v, v_stride,
        );

        match self.encoder.encode(&source) {
            Ok(bitstream) => bitstream.to_vec(),
            Err(_) => vec![],
        }
    }
}

fn bgr_to_nv12(bgr: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut nv12 = vec![0u8; width * height * 3 / 2];
    let y_offset = 0;
    let uv_offset = width * height;

    for y in 0..height {
        for x in 0..width {
            let bgr_idx = (y * width + x) * 3;
            let b = bgr[bgr_idx] as f32;
            let g = bgr[bgr_idx + 1] as f32;
            let r = bgr[bgr_idx + 2] as f32;

            let y_val = 0.299 * r + 0.587 * g + 0.114 * b;
            nv12[y_offset + y * width + x] = y_val as u8;

            if y % 2 == 0 && x % 2 == 0 {
                let u_val = -0.147 * r - 0.289 * g + 0.436 * b + 128.0;
                let v_val = 0.615 * r - 0.515 * g - 0.100 * b + 128.0;
                let uv_idx = uv_offset + (y / 2) * width + x;
                nv12[uv_idx] = u_val as u8;
                nv12[uv_idx + 1] = v_val as u8;
            }
        }
    }
    nv12
}

fn bgr_to_yuv420p(bgr: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut yuv = vec![0u8; width * height * 3 / 2];
    let u_start = width * height;
    let v_start = u_start + (width * height / 4);

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let b = bgr[i * 3] as f32;
            let g = bgr[i * 3 + 1] as f32;
            let r = bgr[i * 3 + 2] as f32;

            yuv[i] = (0.299 * r + 0.587 * g + 0.114 * b) as u8;

            if y % 2 == 0 && x % 2 == 0 {
                let uv_idx = (y / 2) * (width / 2) + (x / 2);
                yuv[u_start + uv_idx] = (-0.169 * r - 0.331 * g + 0.499 * b + 128.0) as u8;
                yuv[v_start + uv_idx] = (0.499 * r - 0.418 * g - 0.0813 * b + 128.0) as u8;
            }
        }
    }
    yuv
}

async fn negotiate_relay_via_nostr(keys: &Keys) -> anyhow::Result<RelayMap> {
    let opts = ClientOptions::new().wait_for_send(true);
    let client = nostr_sdk::Client::builder().signer(keys.clone()).opts(opts).build();
    client.add_relay("wss://relay.damus.io").await?;
    client.connect().await;

    let mut relay_map = RelayMap::default();

    let filter = Filter::new().kind(Kind::GiftWrap).pubkey(keys.public_key()).limit(1);
    client.subscribe(vec![filter], None).await?;

    let mut notifications = client.notifications();
    while let Ok(notification) = notifications.recv().await {
        if let RelayPoolNotification::Event { event, .. } = notification {
            if let Ok(unwrapped) = client.unwrap_gift_wrap(&event).await {
                if unwrapped.rumor.content.contains("best_relay:") {
                    let relay_host = unwrapped.rumor.content.split(':').last().unwrap_or("default");
                    let url = format!("https://{}.tailscale.net", relay_host);
                    if let Ok(url) = iroh::relay::RelayUrl::from_str(&url) {
                        relay_map.add_node(url, iroh::relay::RelayMode::Stun);
                    }
                    break;
                }
            }
        }
    }

    Ok(relay_map)
}

pub async fn start_high_speed_stream(ticket_str: String, room: String) -> anyhow::Result<()> {
    let keys = Keys::generate();
    let relay_map = negotiate_relay_via_nostr(&keys).await.unwrap_or_else(|_| RelayMap::default());

    let endpoint = Endpoint::builder()
        .relay_mode(RelayMode::Custom(relay_map))
        .bind()
        .await?;

    let ticket = iroh::NodeTicket::from_str(&ticket_str).map_err(|e| anyhow::anyhow!(e))?;
    let conn = endpoint.connect_ticket(ticket).await?;

    let (mut session, mut control) = moq_transport::session::run(conn).await?;
    let mut publisher = session.publish(&room).await?;

    // Shared Noise Encryption Key (Simplified for example)
    let noise_key = [0u8; 32];
    let cipher = XChaCha20Poly1305::new(&noise_key.into());

    tokio::spawn(async move {
        let width = 1280;
        let height = 720;
        let mut encoder = VideoEncoder::new(width, height, 30).ok();
        let mut fallback = if encoder.is_none() { Some(FallbackEncoder::new(width, height)) } else { None };

        let start_time = std::time::Instant::now();

        loop {
            if let Some((raw_data, w, h)) = capture_screen(0) {
                let timestamp = start_time.elapsed().as_nanos() as i64 / 100;

                if let Some(enc) = encoder.as_mut() {
                    let nv12_data = bgr_to_nv12(&raw_data, w, h);
                    if let Ok(slices) = enc.encode_frame(&nv12_data, timestamp) {
                        for slice in slices {
                            // 1. Zstd Compression
                            if let Ok(compressed) = encode_all(&slice[..], 10) {
                                // 2. Noise Encryption (ChaCha20Poly1305)
                                let mut nonce = [0u8; 24];
                                rand::rng().fill_bytes(&mut nonce);
                                if let Ok(encrypted) = cipher.encrypt(&nonce.into(), &compressed[..]) {
                                    let mut final_payload = nonce.to_vec();
                                    final_payload.extend_from_slice(&encrypted);
                                    publisher.write(final_payload).await.ok();
                                }
                            }
                        }
                    }
                } else if let Some(enc) = fallback.as_mut() {
                    let packet = enc.encode(&raw_data);
                    if let Ok(compressed) = encode_all(&packet[..], 10) {
                        let mut nonce = [0u8; 24];
                        rand::rng().fill_bytes(&mut nonce);
                        if let Ok(encrypted) = cipher.encrypt(&nonce.into(), &compressed[..]) {
                            let mut final_payload = nonce.to_vec();
                            final_payload.extend_from_slice(&encrypted);
                            publisher.write(final_payload).await.ok();
                        }
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(33)).await;
        }
    });

    Ok(())
}
