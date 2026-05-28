use blake3::{hash, Hasher};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileTreeHash {
    pub root: [u8; 32],
    pub chunks: Vec<[u8; 32]>,
}

impl FileTreeHash {
    pub fn as_bytes(&self) -> &[u8] {
        &self.root
    }
}

pub fn compute_tree_hash(path: impl AsRef<Path>, chunk_size: usize) -> io::Result<FileTreeHash> {
    if chunk_size == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "chunk size must be non-zero",
        ));
    }

    let mut file = File::open(path.as_ref())?;
    let mut buffer = vec![0u8; chunk_size];
    let mut chunks = Vec::new();
    let mut hasher = Hasher::new();

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let chunk_hash = hash(&buffer[..bytes_read]);
        chunks.push(*chunk_hash.as_bytes());
        hasher.update(&buffer[..bytes_read]);
    }

    let root = *hasher.finalize().as_bytes();
    Ok(FileTreeHash { root, chunks })
}
