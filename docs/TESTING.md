# Тесты и фактический статус проверки

[← README](../README.md)

## Статус проверки

**Все этапы проверки (Rust build, 49 unit/integration тестов, 2 doc-теста, Clippy, rustfmt, rustdoc, static preflight и 16 compiled-CLI E2E тестов) успешно выполнены.**

- Toolchain: `rustc 1.85.1 (4eb161250 2025-03-15)` / `cargo 1.85.1` (x86_64 Linux / WSL2).
- `bash scripts/verify.sh` завершился с кодом **0**.
- Форматирование нормализовано (`cargo fmt --all`) и проверено (`cargo fmt --all -- --check`).
- Clippy с `-D warnings` выполнен без замечаний.
- Все 49 тестов (24 gulag, 10 reactor config/report, 9 reactor runtime, 6 synapse queue) и 2 doc-теста пройдены.
- Скомпилированы debug и release бинарники `omsk`.
- 16 black-box E2E тестов пройдены как на debug, так и на release сборках.

> *Исторический контекст подготовки:* В первоначальной изолированной среде подготовки отсутствовал компилятор Rust, поэтому релиз изначально передавался как кандидат реализации с бейджем `unverified`. После развёртывания toolchain 1.85.1 верификация была выполнена в полном объёме без единой ошибки.

## Что проверялось доступными инструментами

Машиночитаемое evidence: [verification.json](verification.json). Вывод статической проверки: [preflight.json](preflight.json).

- Безопасная распаковка, отсутствие archive traversal/symlink entries, отдельный immutable исходный снимок.
- TOML manifests и lockfile: три локальных packages, без registry sources.
- Баланс delimiters и format-string braces в Rust-файлах. **Это не Rust syntax/type checking.**
- Python syntax и выполнение генератора fixtures.
- Hash/length fixtures и независимые ожидания по byte-pattern matching. **Oracle не вызывает Rust scanner.**
- Разбор fixture ELF утилитой GNU readelf и дизассемблирование code fixtures GNU objdump, если указано в evidence.
- Локальные Markdown links, существование модулей/include files, отсутствие executable stubs.
- Контроль совпадения LICENSE с оригиналом и итогового ZIP/source listing.

Ни одна из этих проверок не подменяет сборку или runtime-тесты Rust. Counters «authored» в evidence — количество написанных тестов, не прошедших.

## Полная проверка на Linux с Rust

```bash
bash scripts/verify.sh
```

Нужны Rust/Cargo с rustfmt+Clippy и Python 3.11+. Зависимостей PyPI у test scripts нет. Toolchain 1.85.1 указан в `rust-toolchain.toml`; также желательно проверить актуальный stable.

Скрипт выполняет:

1. Formatter, затем formatting check.
2. Clippy с `-D warnings`, workspace/all-targets, offline/locked.
3. Rust unit/integration tests и doc tests.
4. Debug и release сборку `omsk`.
5. Rustdoc с `-D warnings`.
6. Offline repository preflight.
7. Black-box tests **обоих скомпилированных binaries**.

```bash
cargo test --offline --locked --workspace --all-targets
cargo test --offline --locked --workspace --doc
python3 scripts/e2e.py --binary target/debug/omsk
```

`e2e.py` отвергает отсутствие binary. Он не запускает Python-имитацию scanner и не помечает отсутствие Cargo как skipped success.

## Написанное покрытие

### Scanner / ELF

Все восемь правил и их последние допустимые offsets; 256 ModRM значений для XRSTOR и XRSTORS; one-byte/empty inputs; prefix handling; immediate false-positive semantics; capped storage и точный count; профили; cancellation; stripped ELF; неисполняемые trailing bytes; ошибочные/обрезанные headers; overflow; machine/class/endianness; unsupported numbering/alignment/tails; overlapping/adjacent regions; relocatable PROGBITS; deterministic input mutation.

Header mutation test — regression stress, не полноценный coverage-guided fuzzer и не security certification.

### Channel / runtime

Queue capacity, Full, FIFO ordering, drain/disconnect, blocked sender release, recv timeout, многократная передача между потоками, bounded input, immutable snapshot, symlink rejection, atomic publication, output race/no-clobber, drop cleanup, pipeline order, broken writer cleanup, cancellation и unreadable inputs.

### Compiled CLI black-box

Help/version/rules, fixture outcomes, JSON structure, counts/caps, profiles, mixed file errors, SARIF binary regions и notifications, UTF-8/control filenames, invalid args, limits, regular-file restrictions/FIFO, config precedence, non-executing env parser, no-clobber, output=input, broken pipe, SIGINT/SIGTERM.

SARIF checks покрывают используемую структуру, не являются полной внешней schema certification или тестом загрузки в GitHub.

## Fixtures

[manifest.json](../fixtures/manifest.json) содержит hashes, размеры, ожидаемые exit codes/counts. Перегенерация:

```bash
python3 scripts/make_fixtures.py
```

Fixtures никогда не исполняются. `clean` означает отсутствие **выбранных byte patterns**, а не безопасный native executable. Некоторые ELF преднамеренно минимальны и предназначены только для parser/lint testing.

## Ещё необходимо перед production

- Реальное выполнение всех gates и ревью результатов.
- Fuzzing campaign parser/matcher; при необходимости Miri/санитайзеры на поддерживаемой конфигурации.
- Проверка signal ABI и shutdown на целевой Linux-платформе.
- Реальные linker outputs, ложные совпадения, RSS и throughput.
- Container build/run, OS/stdlib/toolchain advisory review.
- Проверка SARIF consumer и operational rollout сначала без блокировки релизов.
