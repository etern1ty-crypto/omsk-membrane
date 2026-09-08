# Результат ревизии OMSK Membrane

## Выжимка

**Ниша #1:** offline CI/CD byte-pattern gate для контролируемых Linux x86-64 plugins, SDK и вычислительных модулей. Целевая аудитория — platform/security engineers. Альтернативы: raw firmware/code QA и отдельная SPSC-библиотека. Выбранный путь ближе к реальному исходному коду; подтверждённый PMF и платящие клиенты не заявляются.

**Исходник:** Rust 2021 workspace (`gulag`, `synapse`, `reactor`), четыре byte signatures, заголовок без payload и бесконечный loop. MPK/CET, snapshots, io_uring и полноценный guest sandbox в нём отсутствовали.

**Что сделано:** внесён реальный CLI workflow; ELF64/raw parsing; восемь правил; profiles/config; bounded queues; immutable reads; text/JSON/SARIF; complete/error semantics; graceful cancellation; atomic no-clobber output. Registry crates удалены. README, Mermaid, полный docs/, CI, tests и Apache-2.0 сохранены/оформлены.

**Чего нельзя обещать:** компиляция и runtime не подтверждены без Rust toolchain. Это реализационный кандидат, не сертифицированный production-релиз и не sandbox.

## Найденные проблемы

В `docs/AUDIT.md` реестр из 15 пунктов с исходными файлами и строками: неверные security claims; scanner не подключён; Mmap/MmapMut mismatch; отсутствующий data plane; fake ACK; busy-wait/no shutdown; неполный denylist; byte/immediate ambiguity; некорректные logs; dependency advisory; Docker mismatch; слабое покрытие; build garbage; широкие ignore patterns; незакрытые задачи и несогласованные версии.

memmap2 0.9.9 находится в affected range [RUSTSEC-2026-0186](https://rustsec.org/advisories/RUSTSEC-2026-0186.html). Affected methods в исходнике не вызывались, поэтому reachable exploit не установлен. Неиспользуемая библиотека удалена вместе с registry libc dependency.

## Полная выдача

- `omsk-membrane-0.2.0-source.zip` — 65 файлов source-репозитория, включая fixtures и документацию; без target/ и старых исполняемых файлов.
- `OMSK_FULL_SOURCE.md` — полное содержимое каждого файла с точным путём, без многоточий. Binary fixtures представлены целиком в Base64.
- `REVISION_REPORT.md` — эта выжимка; подробные материалы находятся в ZIP.

Листинг проверен обратным восстановлением каждого файла и сравнением SHA-256. ZIP проверен на целостность, набор путей, совпадение содержимого и повторной распаковкой со статической проверкой.

## Проверки

**Фактически выполнены:** TOML/lockfile consistency; lexical delimiter/format-brace checks Rust (НЕ type checking); Python syntax и fixture generator; 12 fixture hashes и независимые byte-pattern expectations; GNU readelf/objdump; JSON Schema через Ajv2020 с 9 cases; YAML structure; локальные docs links; limited source secret patterns; сохранение LICENSE; ZIP/full-source roundtrip.

**Написаны, но не выполнялись:** 49 Rust tests и 16 Python black-box test methods, вызывающих скомпилированный CLI. Rust/Cargo отсутствуют, установка блокируется сетевым DNS. `verify.sh` честно возвращает 127, а E2E не маскирует отсутствие binary.

**Также не выполнялись:** Clippy, rustfmt, rustdoc, Docker build/run, full fuzzing campaign, независимый security review и реальный production pilot.

## Запуск после установки toolchain

```bash
cd omsk-membrane-0.2.0
cargo build --offline --locked --release
./target/release/omsk scan fixtures/clean.elf
```

Полная проверка: `bash scripts/verify.sh` (сначала нормализует форматирование). Для тестового violating artifact ожидается exit 1; для clean — 0. Не исполняйте fixture binaries.

## Контрольные суммы

ZIP SHA-256: `c418cf919e4672dfe667eff8b3a5e4dd1110fde04fdf64cb8a4853189b376205`

Full source SHA-256: `718cc80e0b293707a8e124e0174560e47483e4bfc02bb3047297d15bab3eb3e8`

Отсутствие сигнатур означает лишь отсутствие выбранных byte patterns в просмотренных регионах. Instruction boundaries, malicious behavior и безопасность исполнения из этого не следуют.
