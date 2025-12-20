use std::sync::atomic::AtomicU64;

// [OMSK] LAYER II: SHARED MEMORY GEOMETRY
// Cache Line: 128 bytes (L2/L3 prefetch optimization)
#[repr(C)]
#[repr(align(128))]
pub struct SynapseHeader {
    // Producer Cache Line (Host writes here)
    pub head: AtomicU64,
    _pad_head: [u8; 120], // 128 - 8 = 120 bytes padding

    // Consumer Cache Line (Guest writes here)
    pub tail: AtomicU64,
    _pad_tail: [u8; 120],
}

impl SynapseHeader {
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            _pad_head: [0; 120],
            tail: AtomicU64::new(0),
            _pad_tail: [0; 120],
        }
    }
}