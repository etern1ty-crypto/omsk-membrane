/// [OMSK] LAYER V: THE LAW
/// The Gulag Verifier ensures that the guest code does not contain any forbidden opcodes.
///
/// FORBIDDEN OPCODES:
/// - 0F 01 EF (WRPKRU) - Protects MPK keys
/// - 0F 05 (SYSCALL)   - Enforces Zero-Syscall policy
/// - 0F 31 (RDTSC)     - Prevents timing attacks
/// - CD 80 (INT 0x80)  - Legacy Syscall (32-bit compat)
pub fn scan(code: &[u8]) -> bool {
    let len = code.len();
    if len < 2 {
        return true;
    }

    let mut i = 0;
    while i < len - 1 {
        let byte = code[i];
        let next = code[i+1];

        // 1. ПРОВЕРКА 0F (СОВРЕМЕННЫЕ ИНСТРУКЦИИ)
        if byte == 0x0F {
            if next == 0x05 { return false; } // SYSCALL
            if next == 0x31 { return false; } // RDTSC
            if next == 0x01 {                 // WRPKRU
                if i + 2 < len && code[i+2] == 0xEF {
                    return false;
                }
            }
        }
        
        // 2. ПРОВЕРКА CD (УСТАРЕВШИЕ ПРЕРЫВАНИЯ)
        if byte == 0xCD {
            if next == 0x80 { return false; } // INT 0x80
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
        let code = [0x90, 0x90, 0x55, 0x48, 0x89, 0xE5]; 
        assert!(scan(&code));
    }

    #[test]
    fn test_ban_syscall() {
        let code = [0x90, 0x0F, 0x05, 0xC3]; 
        assert!(!scan(&code));
    }
    
    #[test]
    fn test_ban_rdtsc() {
        let code = [0x48, 0x0F, 0x31]; 
        assert!(!scan(&code));
    }

    #[test]
    fn test_ban_wrpkru() {
        let code = [0xB8, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x01, 0xEF]; 
        assert!(!scan(&code));
    }

    #[test]
    fn test_ban_int80() {
        let code = [0x90, 0xCD, 0x80, 0xC3]; 
        assert!(!scan(&code));
    }
}
