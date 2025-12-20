use gulag;

#[test]
fn audit_syscall_injection() {
    let payload = [0x90, 0x90, 0x0F, 0x05, 0x90, 0xC3];
    assert!(!gulag::scan(&payload), " SYSCALL (0F 05) was allowed!");
}

#[test]
fn audit_wrpkru_theft() {
    let payload = [0x0F, 0x01, 0xEF];
    assert!(!gulag::scan(&payload), " WRPKRU (0F 01 EF) was allowed!");
}

#[test]
fn audit_false_positive_check() {
    let payload = [0x83, 0xC0, 0x05];
    assert!(gulag::scan(&payload), " Valid code was blocked!");
}

#[test]
fn exploit_legacy_interrupt() {
    println!("[RED TEAM] Testing INT 0x80...");
    let payload = [0xCD, 0x80]; // Старый 32-битный Syscall
    
    // ЕСЛИ scan возвращает true (безопасно), ЗНАЧИТ МЫ ПРОИГРАЛИ.
    if gulag::scan(&payload) {
        panic!(" CRITICAL: Verifier BLIND to Legacy Syscall (CD 80)!");
    }
}
