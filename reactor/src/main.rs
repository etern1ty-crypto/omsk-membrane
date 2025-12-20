mod io_pump;
// mod guest; // Unused for now

use std::sync::Arc;
use synapse::SynapseHeader;
use io_pump::IoPump;


fn main() {
    println!("[OMSK] REACTOR INIT");
    
    // 1. Allocate Shared Memory (Simulated for this stage via Heap, ideally shm_open)
    // We abide by the 128-byte alignment rule of SynapseHeader.
    // In a real run this would be: let shm = Guest::new(4096)...
    let header = Arc::new(SynapseHeader::new());

    println!("[OMSK] LAYER II: ACTIVE (Address: {:p})", header);
    println!("[OMSK] LAYER III: ACTIVE");

    // 2. Start IO Pump
    let pump = IoPump::new(header.clone());
    pump.poll(); // Diverges (Loop forever)
}




