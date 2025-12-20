use memmap2::{Mmap, MmapOptions};
use std::io;

pub struct Guest {
    memory: Mmap,
}

impl Guest {
    pub fn new(size: usize) -> io::Result<Self> {
        let memory = MmapOptions::new()
            .len(size)
            .map_anon()?;
        
        Ok(Self { memory })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.memory[..]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.memory[..]
    }
}
