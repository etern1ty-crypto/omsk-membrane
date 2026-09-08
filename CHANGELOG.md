# Changelog

## 0.2.0 — implementation candidate, 2026-09-08

### Product

- Нативный VM prototype переориентирован на offline ELF64/raw byte-pattern policy gate.
- Убраны неподтверждённые isolation и performance claims.
- Добавлена явная модель угроз и проверяемая гипотеза рынка вместо обещания подтверждённого PMF.

### Added / changed

- Реальный `omsk scan/rules/config` CLI, eight-rule policy API, ELF parser.
- Bounded SPSC endpoints вместо одного atomic header.
- Immutable file input, finite worker pipeline, cancellation and joining.
- Text/JSON/SARIF, file offsets, bounded findings with exact counts.
- Atomic no-clobber report output и stderr progress.
- `.env.example`, multi-stage runtime Dockerfile, CI и regression suites.
- README и полноценная документация по архитектуре, конфигурации, API, deployment, audit и testing.
- Исторические crate names сохранены; API breaking changes допустимы для 0.x.

### Removed

- Registry dependencies `memmap2`/`libc`; file mmap path и fake guest ACTIVE state.
- Busy-wait loop, fake acknowledgement и необработанные command stubs.
- Из source distribution исключены `target/`, Windows build artifacts и caches.

### Breaking

- Булев `gulag::scan(&[u8])` заменён structured `scan` с format/policy/options/cancellation.
- `SynapseHeader` заменён `bounded`, `Producer`, `Consumer`.
- Binary называется `omsk` вместо демонстрационного бесконечного `reactor`.
- CLI scan официально ограничен 64-bit Linux; PE/Mach-O/other architectures — explicit error.
- JSON schema v1: virtual addresses — hex strings, file offsets — числа.

### Verification

Rust compilation, Rust tests, Clippy, formatting, E2E и container execution **не подтверждены** в исходной среде выдачи. Релиз не следует обозначать production-ready до выполнения [release gates](docs/DEPLOYMENT.md).
