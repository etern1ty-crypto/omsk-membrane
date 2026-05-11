<img src="https://capsule-render.vercel.app/api?type=waving&color=0:1a1b27,50:E74C3C,100:1a1b27&height=200&section=header&text=OMSK%20MEMBRANE&fontSize=50&fontColor=FFFFFF&fontAlignY=35&desc=Hardened%20Virtual%20Membrane%20%E2%80%94%20Zero-Syscall%20Architecture&descSize=16&descColor=F5B7B1&descAlignY=55&animation=fadeIn" width="100%"/>

<div align="center">

[![Rust](https://img.shields.io/badge/rust-stable-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-red?style=flat-square)](LICENSE)
[![Linux 6.8+](https://img.shields.io/badge/kernel-Linux_6.8+-FCC624?style=flat-square&logo=linux&logoColor=black)]()
[![Version](https://img.shields.io/badge/version-5.0-blue?style=flat-square)]()
[![Architecture](https://img.shields.io/badge/arch-x86__64-lightgrey?style=flat-square)]()

**🇷🇺 [Русский](#-описание) · 🇬🇧 [English](#-overview)**

</div>

---

> *"Engineering is the only real magic."* — Ivan Ivanovich

---

## 🇬🇧 Overview

**OMSK Virtual Membrane V5.0** is a hardened virtualization layer that replaces reactive overhead with **constructive geometry**.

We do not check bounds. We align memory so bounds do not matter.
We do not filter syscalls. We remove the ability to issue them.

### Performance Targets

| Metric | Target |
|:---|:---|
| **Cold Boot** | < 60µs |
| **I/O Throughput** | 5M+ OPS (batch-free) |
| **CPU Overhead** | < 2% |

### Architecture

#### Layer II — The Physical Invariant

Communication via `synapse`: a **SPSC (Single-Producer-Single-Consumer)** ring buffer in shared memory.

- **Alignment:** 128-byte cache lines
- **Semantics:** Acquire/Release atomics
- **Cost:** 0 syscalls

#### Layer III — The Silicon Shield

| Technology | Purpose |
|:---|:---|
| **CET** | Shadow Stacks + Indirect Branch Tracking |
| **MPK** | Memory Protection Keys (PKRU owned by host) |
| **CoW** | Frozen snapshots via `userfaultfd` → 50µs cold starts |

#### Layer V — The Law

The `gulag` verifier rejects **illegal opcodes** before execution:

| Opcode | Instruction | Reason |
|:---|:---|:---|
| `0F 01 EF` | `WRPKRU` | Guest must not modify memory protection keys |
| `0F 05` | `SYSCALL` | Guest has no kernel interface |
| `0F 31` | `RDTSC` | Timing attacks / side-channel prevention |

### Build

```bash
cargo build --release
```

Requires Linux 6.8+ kernel and stable Rust toolchain.

### Tech Stack

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Linux](https://img.shields.io/badge/linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)

---

## 🇷🇺 Описание

**OMSK Virtual Membrane V5.0** — это закалённый слой виртуализации, заменяющий реактивную нагрузку **конструктивной геометрией**.

Мы не проверяем границы. Мы выравниваем память так, чтобы границы не имели значения.
Мы не фильтруем системные вызовы. Мы убираем возможность их совершать.

### Целевые Показатели

| Метрика | Цель |
|:---|:---|
| **Холодный старт** | < 60µs |
| **I/O пропускная способность** | 5M+ OPS (без батчинга) |
| **Нагрузка CPU** | < 2% |

### Архитектура

#### Слой II — Физический Инвариант

Коммуникация через `synapse`: **SPSC** кольцевой буфер в разделяемой памяти.

- **Выравнивание:** 128-байтные кэш-линии
- **Семантика:** Acquire/Release атомики
- **Стоимость:** 0 системных вызовов

#### Слой III — Кремниевый Щит

| Технология | Назначение |
|:---|:---|
| **CET** | Shadow Stacks + Indirect Branch Tracking |
| **MPK** | Memory Protection Keys (PKRU принадлежит хосту) |
| **CoW** | Замороженные снимки через `userfaultfd` → 50µs холодный старт |

#### Слой V — Закон

Верификатор `gulag` отклоняет **нелегальные опкоды** до исполнения:

| Опкод | Инструкция | Причина |
|:---|:---|:---|
| `0F 01 EF` | `WRPKRU` | Гость не должен менять ключи защиты памяти |
| `0F 05` | `SYSCALL` | У гостя нет интерфейса ядра |
| `0F 31` | `RDTSC` | Предотвращение timing-атак |

### Сборка

```bash
cargo build --release
```

Требуется ядро Linux 6.8+ и стабильный Rust toolchain.

---

<div align="center">

### License

Apache 2.0

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:1a1b27,50:E74C3C,100:1a1b27&height=80&section=footer" width="100%"/>

</div>
