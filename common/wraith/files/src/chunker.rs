use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub struct FileChunker {
    file: File,
    chunk_size: usize,
    file_size: u64,
    num_chunks: u64,
}

impl FileChunker {
    pub fn new(path: impl AsRef<Path>, chunk_size: usize) -> io::Result<Self> {
        if chunk_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk size must be non-zero",
            ));
        }

        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();
        let num_chunks = if file_size == 0 {
            0
        } else {
            (file_size + chunk_size as u64 - 1) / chunk_size as u64
        };

        Ok(Self {
            file,
            chunk_size,
            file_size,
            num_chunks,
        })
    }

    pub fn num_chunks(&self) -> u64 {
        self.num_chunks
    }

    pub fn read_chunk_at(&mut self, chunk_index: u64) -> io::Result<Vec<u8>> {
        if chunk_index >= self.num_chunks {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk index out of bounds",
            ));
        }

        let offset = chunk_index * self.chunk_size as u64;
        if offset >= self.file_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk offset is out of bounds",
            ));
        }

        self.file.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; self.chunk_size];
        let bytes_read = self.file.read(&mut buffer)?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    }
}

pub struct FileReassembler {
    file: File,
    file_size: u64,
    chunk_size: usize,
}

impl FileReassembler {
    pub fn new(path: impl AsRef<Path>, file_size: u64, chunk_size: usize) -> io::Result<Self> {
        if chunk_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk size must be non-zero",
            ));
        }

        let file_path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(&file_path)?;
        file.set_len(file_size)?;

        Ok(Self {
            file,
            file_size,
            chunk_size,
        })
    }

    pub fn write_chunk(&mut self, chunk_index: u64, chunk_data: &[u8]) -> io::Result<()> {
        let offset = chunk_index.saturating_mul(self.chunk_size as u64);
        if offset > self.file_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk offset is out of bounds",
            ));
        }

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(chunk_data)?;
        Ok(())
    }
}
