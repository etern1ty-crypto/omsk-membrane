/// [OMSK] LAYER V: THE LAW
/// The Gulag Verifier ensures that the guest code does not contain any forbidden opcodes.
/// 
/// FORBIDDEN OPCODES:
/// - 0F 01 EF (WRPKRU) - Protects MPK keys
/// - 0F 05 (SYSCALL)   - Enforces Zero-Syscall policy
/// - 0F 31 (RDTSC)     - Prevents timing attacks
pub fn scan(code: &[u8]) -> bool {
    let len = code.len();
    if len < 2 {
        return true;
    }

    let mut i = 0;
    while i < len - 1 {
        // Check for 0F prefix
        if code[i] == 0x0F {
            let next = code[i+1];
            
            // CHECK: SYSCALL (0F 05)
            if next == 0x05 {
                return false;
            }

            // CHECK: RDTSC (0F 31)
            if next == 0x31 {
                return false;
            }

            // CHECK: WRPKRU (0F 01 EF)
            if next == 0x01 {
                if i + 2 < len && code[i+2] == 0xEF {
                    return false;
                }
            }
        }
        i += 1;
    }
    
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_safe_code() {
        let code = [0x90, 0x90, 0x55, 0x48, 0x89, 0xE5]; // NOP, PUSH RBP, MOV RBP, RSP
        assert!(scan(&code));
    }

    #[test]
    fn test_ban_syscall() {
        let code = [0x90, 0x0F, 0x05, 0xC3]; // NOP, SYSCALL, RET
        assert!(!scan(&code));
    }

    #[test]
    fn test_ban_rdtsc() {
        let code = [0x48, 0x0F, 0x31]; // REX.W RDTSC
        assert!(!scan(&code));
    }

    #[test]
    fn test_ban_wrpkru() {
        let code = [0xB8, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x01, 0xEF]; // MOV EAX, 0; WRPKRU
        assert!(!scan(&code));
    }

    #[test]
    fn test_partial_match_safe() {
        // 0F 01 but not EF (e.g. LGDT / LIDT which are privileged checked elsewhere, or Monitor)
        // For this specific scanner we only ban WRPKRU explicitly.
        // 0F 01 C3 = VMRESUME (just an example of 0F 01 prefix)
        let code = [0x0F, 0x01, 0xC3]; 
        assert!(scan(&code));
    }
}
