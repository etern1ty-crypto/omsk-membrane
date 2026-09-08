# Технический аудит исходного архива

[← README](../README.md)

## Область и достоверность

Проверены все исходные текстовые файлы архива `omsk-membrane-main.zip`, manifests, Dockerfile, README, задачи и сохранённые compiler diagnostics. `target/` не запускался и не принимался за доказательство работоспособности текущего исходника.

Номера строк ниже относятся к **исходному архиву**, не к переработанным файлам. Исходные checksums и размеры: [original-manifest.json](original-manifest.json). Состояние новых проверок: [TESTING.md](TESTING.md).

Rust compiler в среде отсутствовал. Выводы о типах, архитектуре и логике — source inspection; старые compiler logs являются вспомогательными артефактами. Не утверждается, что исходная версия была скомпилирована или что каждая проблема была динамически воспроизведена.

## Реестр дефектов и решений

| ID / приоритет | Исходный файл и строки | Проблема / влияние | Изменение в 0.2.0 | Проверка / граница |
| --- | --- | --- | --- | --- |
| A01 / critical | `README.md:23–26, 48–52, 81–84, 106–110`; `reactor/src/main.rs:17–18` | Обещания hardened VM, отсутствия syscalls и активного hardware shield не соответствуют коду | Изменено назначение на неисполняющий artifact lint; явная threat model; удалены ложные ACTIVE-сообщения | Изоляция не «починена», а исключена из заявленных возможностей |
| A02 / high | `reactor/src/main.rs:9–22` | Scanner `gulag` не вызывается; нет input/CLI/result workflow | Полный конечный CLI scan с per-file result и exit codes | Rust/E2E tests написаны, запуск заблокирован toolchain |
| A03 / high | `reactor/src/guest.rs:4, 10–14, 21–22`; `main.rs:2` | Anonymous mmap возвращает `MmapMut`, хранится как `Mmap`; попытка mutable access через read-only mapping. Модуль выключен из main | Mmap удалён; `guest.rs` теперь bounded immutable snapshot reader | Старые release diagnostics содержат E0308/E0596; текущая default main не подключала этот файл |
| A04 / high | `synapse/src/lib.rs:7–15` | Есть только head/tail; отсутствуют payload slots и producer/consumer протокол | Настоящие bounded channels с уникальными endpoints и disconnect semantics | API/queue tests; это не lock-free IPC |
| A05 / high | `reactor/src/io_pump.rs:28–35` | Head подтверждается через `tail=head` без чтения и обработки команды; фактический результат отсутствует | Worker реально читает файл, разбирает, сканирует и выдаёт outcome | Нет подтверждений необработанной работы |
| A06 / medium | `reactor/src/io_pump.rs:18, 38–46`; `main.rs:22` | Бесконечный busy-wait, редкий sleep, нет штатного shutdown | Blocking/backpressured queues, recv timeout, cancellation, thread join | CPU baseline не измерен; исправлена архитектура ожидания |
| A07 / high | `gulag/src/lib.rs:20–34` | Неполный даже для исходного намерения список: пропускаются SYSENTER/RDTSCP; state restore не учтён | Восемь named rules, включая memory XRSTOR/XRSTORS signatures | Это всё ещё конечный список сигнатур, не полный ISA verifier |
| A08 / high | `gulag/src/lib.rs:9–39`; `README.md:56–62` | Булев ответ подаётся как безопасность; `B8 0F 05 00 00 C3` содержит сигнатуру в MOV immediate | Структурированный результат, explicit byte-lint semantics, анализ только selected ELF regions | **Ложные instruction-level совпадения внутри executable bytes остаются ограничением**; предлагается review в disassembler |
| A09 / medium | `reactor/src/main.rs:10, 17–18`; `io_pump.rs:19` | Необоснованные security-state logs, указатель heap в stdout, нет report/data channel separation | Escaped progress в stderr; только report в stdout; без heap addresses | JSON/escaping/E2E tests |
| A10 / medium | `Cargo.lock:15–19`; `reactor/Cargo.toml:9–10` | `memmap2 0.9.9` входит в affected range RUSTSEC-2026-0186; зависимости не используются активным сценарием | `memmap2` и registry `libc` удалены вместе с ненужным mapping path | Reachable exploit не установлен: affected range methods в исходнике не вызываются |
| A11 / medium | `Dockerfile:2, 6–19, 27`; `README.md:70, 128` | Floating builder/nightly, ненужные LLVM tools, CMD только собирает, нет runtime image | Multi-stage stable baseline build и непривилегированный CLI runtime | Container recipe подготовлен; Docker build не выполнялся |
| A12 / medium | `gulag/tests/audit.rs:3–29`; `gulag/src/lib.rs:42–75` | Тестируется только маленький scanner; нет input/queue/config/lifecycle/output contracts | Regression tests, malformed ELF/header mutation, runtime I/O tests, compiled-CLI black-box suite | Написанные тесты не выданы за прошедшие |
| A13 / medium | `target/**` в ZIP | В архив включены Windows `.exe`, `.pdb`, incremental metadata и устаревшие diagnostics | Новый source ZIP исключает `target/` и executable build artifacts | Проверка состава ZIP и checksums |
| A14 / low | `.gitignore:1–5` | BOM и широкое `*.txt` скрывают полезные текстовые файлы; ignore не удаляет уже попавшие в архив artifacts | Узкие patterns, отдельный `.dockerignore`, text EOL policy | Репозиторий не содержит build cache |
| A15 / medium | `task.md:3–13`; `README.md:8, 28–34`; manifests `version=0.1.0` | Незакрытые io_uring/Guest задачи и несогласованные версии/производительность | Единая version 0.2.0, закрытый product scope, реальные docs/config; старые обещания удалены | io_uring/MPK/CET/snapshots не объявляются реализованными |

## Приоритет внедрения

1. **Сначала убрать опасное неверное обещание безопасности** и выбрать неисполняющий сценарий.
2. Заменить отсутствующий data plane и dormant Guest на полноценный input → scan → report workflow.
3. Добавить format/size/path validation, bounded memory, output/error contracts и shutdown.
4. Удалить ненужные registry dependencies и build garbage.
5. Подготовить regression suites, CI и документацию; блокировать production release до фактического compiler/E2E gate.

Все перечисленные изменения внесены в исходники. Термин «внесено» не равен «динамически проверено»: execution evidence приведён отдельно.

## Проверка opcode-контрпримеров

| Bytes | Исходный matcher | Что следует из этого |
| --- | --- | --- |
| `0F 34` | Не находил совпадение | SYSENTER отсутствовал в denylist |
| `0F 01 F9` | Не находил совпадение | RDTSCP отсутствовал в timing blacklist |
| `0F AE 28` | Не находил совпадение | XRSTOR memory form не учитывался |
| `B8 0F 05 00 00 C3` | Находил `0F 05` | Byte search не равен instruction decoding |

Для этих исходных patterns выполнена независимая Python-проверка; это не замена исполнения исходного Rust. Fixtures и GNU objdump используются как дополнительная проверка ожидаемой семантики данных. Сам fixture код никогда не исполняется.

## Dependency advisory

[RUSTSEC-2026-0186](https://rustsec.org/advisories/RUSTSEC-2026-0186.html), опубликованный 2026-06-22, отмечает unchecked pointer arithmetic в range methods `memmap2`; patched `>=0.9.11`. Исходная `0.9.9` находится в affected version range.

Это advisory типа **INFO Unsound**, не автоматически доказанная RCE в этом проекте. В исходнике нет вызовов `advise_range`, `unchecked_advise_range`, `flush_range` или `flush_async_range`. Предпочтительное решение здесь — удалить неиспользуемую библиотеку, а не оставить лишнюю dependency и просто поднять версию.

По `libc` не заявляется «уязвимостей нет»: полного сканирования актуальной advisory DB через cargo-audit не было. Новый lockfile содержит только три локальных crates. OS libc/stdlib/compiler/container supply chain по-прежнему требует отдельной проверки.

## Secrets и опасное поведение

В просмотренном исходном коде/manifest/config не обнаружены hardcoded credentials, private keys или сетевые endpoints для выгрузки данных. Это результат обзора предоставленных файлов, не исторический secret scan git history: `.git` отсутствовал.

Новый CLI не выполняет shell, не запускает artifact code, не пишет inputs. Input final symlinks, устройства и FIFO запрещены; на Linux open использует O_NOFOLLOW/O_NONBLOCK. Это не containment для атакующего, полностью контролирующего filesystem/родительские директории; production scanner должен работать в собственном restricted runner.

## Остаточные риски перед production

- Нет подтверждённой Rust compilation, Clippy, E2E и container build в этой среде.
- Нет независимого аудита FFI, formal verification, Miri/TSan и полноценного fuzzing campaign.
- Byte-level false positives, unknown ISA encodings, dynamic/JIT/generated code, dependencies и runtime memory policy не покрываются.
- Для release-blocking внедрения нужны реальные артефакты, review шума и зафиксированная политика.

Не заявляется, что «все возможные баги устранены» или что проект готов безопасно исполнять недоверенные программы.
