# OMSK MEMBRANE: REALIZATION PROTOCOL

- [ ] **GULAG (The Law)**
    - [ ] Implement `Scanner::verify` for forbidden opcodes (0F 01 EF, 0F 05, 0F 31) <!-- id: 1 -->
    - [ ] Add unit tests for opcode detection <!-- id: 2 -->

- [ ] **REACTOR (The Host)**
    - [ ] Implement `IoPump` with `io_uring` (using `io-uring` crate or raw bindings) <!-- id: 3 -->
    - [ ] Implement `Guest` struct for memory mapping <!-- id: 4 -->
    - [ ] Wire up `main.rs` to initialize Synapse and start Pump <!-- id: 5 -->

- [ ] **INTEGRATION**
    - [ ] Verify 0-syscall loop behavior <!-- id: 6 -->
