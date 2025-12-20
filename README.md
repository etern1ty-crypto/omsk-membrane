# OMSK VIRTUAL MEMBRANE (V5.0)

> "Engineering is the only real magic." — Ivan Ivanovich

## STATUS: ARCHITECTURAL LOCK
**PROTOCOL:** STRICT_REALITY
**KERNEL:** LINUX 6.8+

## EXECUTIVE DIRECTIVE
OMSK-VM is a rejection of modern "soft" virtualization. It replaces reactive overhead with **constructive geometry**.

We do not check bounds. We align memory so bounds do not matter.
We do not filter syscalls. We remove the ability to issue them.

## THE ARCHITECTURE

### I. THE PHYSICAL INVARIANT (Layer II)
Communication occurs via `synapse`: a **Single-Producer-Single-Consumer (SPSC)** ring buffer residing in shared memory. 
- **Alignment:** 128-byte cache lines.
- **Semantics:** Acquire/Release. 
- **Cost:** 0 Syscalls.

### II. THE SILICON SHIELD (Layer III)
- **CET:** Shadow Stacks + Indirect Branch Tracking.
- **MPK:** Memory Protection Keys (PKRU) owned by Host.
- **CoW:** Frozen Snapshots via `userfaultfd` for 50µs cold starts.

### III. THE LAW (Layer V)
The `gulag` verifier acts before execution.
It rejects **illegal opcodes**:
- `0F 01 EF` (WRPKRU)
- `0F 05` (SYSCALL)
- `0F 31` (RDTSC)

## PERFORMANCE
| Metric | OMSK Target |
| :--- | :--- |
| **Boot** | < 60µs |
| **I/O** | 5M+ OPS (Batch-free) |
| **Overhead** | < 2% |

## BUILDING
You are the architect now. 
`cargo build --release`

## LICENSE
APACHE 2.0
We provide the weapon. You provide the target.
