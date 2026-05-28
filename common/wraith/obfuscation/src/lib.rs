use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum ObfuscationError {
    InvalidData(String),
    Io(std::io::Error),
}

impl Display for ObfuscationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObfuscationError::InvalidData(msg) => write!(f, "Invalid data: {msg}"),
            ObfuscationError::Io(err) => write!(f, "IO error: {err}"),
        }
    }
}

impl Error for ObfuscationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ObfuscationError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ObfuscationError {
    fn from(err: std::io::Error) -> Self {
        ObfuscationError::Io(err)
    }
}

pub struct TlsRecordWrapper;

impl TlsRecordWrapper {
    pub fn new() -> Self {
        Self
    }

    pub fn wrap(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 5);
        out.extend_from_slice(&[0x17, 0x03, 0x03]);
        let len = (data.len() as u16).to_be_bytes();
        out.extend_from_slice(&len);
        out.extend_from_slice(data);
        out
    }

    pub fn unwrap(&self, data: &[u8]) -> Result<Vec<u8>, ObfuscationError> {
        if data.len() < 5 {
            return Err(ObfuscationError::InvalidData("TLS frame too short".into()));
        }
        if data[0..3] != [0x17, 0x03, 0x03] {
            return Err(ObfuscationError::InvalidData("Invalid TLS header".into()));
        }
        let len = u16::from_be_bytes([data[3], data[4]]) as usize;
        if data.len() != 5 + len {
            return Err(ObfuscationError::InvalidData("TLS length mismatch".into()));
        }
        Ok(data[5..].to_vec())
    }
}

pub struct WebSocketFrameWrapper {
    masked: bool,
}

impl WebSocketFrameWrapper {
    pub fn new(masked: bool) -> Self {
        Self { masked }
    }

    pub fn wrap(&self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 6);
        out.push(0x82); // binary frame
        if self.masked {
            out.push(0x80 | (data.len() as u8));
            out.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            out.extend_from_slice(data);
        } else {
            out.push(data.len() as u8);
            out.extend_from_slice(data);
        }
        out
    }

    pub fn unwrap(&self, data: &[u8]) -> Result<Vec<u8>, ObfuscationError> {
        if data.len() < 2 || data[0] != 0x82 {
            return Err(ObfuscationError::InvalidData("Invalid WebSocket frame".into()));
        }
        let masked = data[1] & 0x80 != 0;
        let len = (data[1] & 0x7F) as usize;
        let payload_offset = if masked { 6 } else { 2 };
        if data.len() != payload_offset + len {
            return Err(ObfuscationError::InvalidData("WebSocket length mismatch".into()));
        }
        Ok(data[payload_offset..].to_vec())
    }
}

pub struct DohTunnel {
    endpoint: String,
}

impl DohTunnel {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub fn create_dns_query(&self, host: &str, data: &[u8]) -> Result<Vec<u8>, ObfuscationError> {
        let mut out = Vec::with_capacity(host.len() + data.len() + 4);
        out.extend_from_slice(host.as_bytes());
        out.push(0);
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(data);
        Ok(out)
    }

    pub fn parse_dns_response(&self, data: &[u8]) -> Result<Vec<u8>, ObfuscationError> {
        if data.is_empty() {
            return Err(ObfuscationError::InvalidData("DoH response too short".into()));
        }
        let terminator = data.iter().position(|&b| b == 0).ok_or_else(|| {
            ObfuscationError::InvalidData("DoH response missing host terminator".into())
        })?;
        if data.len() < terminator + 5 {
            return Err(ObfuscationError::InvalidData("DoH response too short".into()));
        }
        let length = u32::from_be_bytes([
            data[terminator + 1],
            data[terminator + 2],
            data[terminator + 3],
            data[terminator + 4],
        ]) as usize;
        let payload_offset = terminator + 5;
        if data.len() != payload_offset + length {
            return Err(ObfuscationError::InvalidData("DoH response length mismatch".into()));
        }
        Ok(data[payload_offset..].to_vec())
    }
}
