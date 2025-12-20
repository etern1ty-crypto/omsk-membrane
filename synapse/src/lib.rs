// [OMSK] LAYER II: THE PHYSICAL REALITY
// CACHE ALIGNMENT IS NON-NEGOTIABLE.

use std::sync::atomic::AtomicU32;

// Must be power of 2
pub const RING_SIZE: usize = 64 * 1024; 

#[repr(C, align(128))]
pub struct SynapseHeader {
    // WRITTEN BY HOST / READ BY GUEST
    // CACHE LINE 0
    pub head: AtomicU32,
    pub host_state: u32,
    _pad_producer: [u8; 120], 

    // WRITTEN BY GUEST / READ BY HOST
    // CACHE LINE 1
    pub tail: AtomicU32,
    pub guest_fault: u32,
    _pad_consumer: [u8; 120],
}

#[repr(C, align(4096))] // PAGE ALIGNED DATA PLANE
pub struct DataPlane {
    pub buffer: [u8; RING_SIZE],
}

// IF THIS FAILS, THE UNIVERSE IS BROKEN.
const _: () = assert!(std::mem::align_of::<SynapseHeader>() == 128);
