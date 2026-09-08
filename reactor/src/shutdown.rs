//! Cooperative cancellation. This is the only module containing unsafe code.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

static REQUESTED_SIGNAL: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Default)]
pub struct Cancellation {
    local: Arc<AtomicBool>,
}

impl Cancellation {
    pub fn cancel(&self) {
        self.local.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.local.load(Ordering::Relaxed) || requested_exit_code().is_some()
    }
}

pub fn requested_exit_code() -> Option<u8> {
    match REQUESTED_SIGNAL.load(Ordering::Relaxed) {
        2 => Some(130),
        15 => Some(143),
        _ => None,
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod platform {
    use super::*;
    use std::os::raw::c_int;

    const SIGINT: c_int = 2;
    const SIGTERM: c_int = 15;
    const SIG_ERR: usize = usize::MAX;

    extern "C" {
        // Linux libc ABI: signal handler values (including SIG_DFL/SIG_IGN)
        // fit in a pointer-sized word. Supported only on 64-bit Linux.
        fn signal(number: c_int, handler: usize) -> usize;
    }

    extern "C" fn handler(number: c_int) {
        // No allocation, locks, panics or I/O in the signal handler.
        let _ = REQUESTED_SIGNAL.compare_exchange(0, number, Ordering::Relaxed, Ordering::Relaxed);
    }

    pub struct SignalGuard {
        previous_int: usize,
        previous_term: usize,
    }

    impl SignalGuard {
        /// Install once in the CLI before creating threads. Do not install from
        /// a library host: process-wide signal disposition belongs to the host.
        pub fn install() -> io::Result<Self> {
            // SAFETY: the extern C handler has process lifetime, only accesses
            // a lock-free AtomicI32, and matches the Linux signal ABI. Both
            // signal numbers are valid; no Rust references cross this FFI.
            let previous_int = unsafe { signal(SIGINT, handler as *const () as usize) };
            if previous_int == SIG_ERR {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: same handler and ABI invariants as above.
            let previous_term = unsafe { signal(SIGTERM, handler as *const () as usize) };
            if previous_term == SIG_ERR {
                let error = io::Error::last_os_error();
                // SAFETY: restore the value returned by libc for SIGINT.
                unsafe { signal(SIGINT, previous_int) };
                return Err(error);
            }
            Ok(Self {
                previous_int,
                previous_term,
            })
        }
    }

    impl Drop for SignalGuard {
        fn drop(&mut self) {
            // SAFETY: these are the previous libc handler values. Main drops
            // this guard only after the worker/feeder threads are joined.
            unsafe {
                signal(SIGINT, self.previous_int);
                signal(SIGTERM, self.previous_term);
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub use platform::SignalGuard;

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub struct SignalGuard;

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl SignalGuard {
    pub fn install() -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the CLI currently supports 64-bit Linux only",
        ))
    }
}
