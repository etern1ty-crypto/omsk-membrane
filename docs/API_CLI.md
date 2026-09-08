# API / CLI справочник

[← README](../README.md)

## Команды

```text
omsk scan [OPTIONS] [--] FILE [FILE ...]
omsk rules [--format text|json]
omsk config
omsk --help
omsk --version
```

`scan` выполняет одну конечную задачу. `rules` печатает полный каталог правил, независимо от профиля. `config` печатает пригодный для сохранения env example. `--help` не требует входных файлов. Неизвестные аргументы, повторённые опции и отсутствие значения после опции дают код `2`.

Все flags, defaults и precedence: [CONFIGURATION.md](CONFIGURATION.md).

## Правила

| ID | Имя | Сигнатура | Назначение |
| --- | --- | --- | --- |
| OMSK001 | `syscall` | `0F 05` | Direct system-call entry |
| OMSK002 | `sysenter` | `0F 34` | Fast legacy syscall entry |
| OMSK003 | `int80` | `CD 80` | Legacy interrupt syscall |
| OMSK004 | `wrpkru` | `0F 01 EF` | Запись PKRU |
| OMSK005 | `xrstor` | `0F AE /5`, memory ModRM | Restore extended state, включая возможный PKRU |
| OMSK006 | `xrstors` | `0F C7 /3`, memory ModRM | Supervisor extended-state restore |
| OMSK007 | `rdtsc` | `0F 31` | Чтение timestamp counter |
| OMSK008 | `rdtscp` | `0F 01 F9` | Чтение timestamp counter и processor ID |

XRSTOR/XRSTORS `/n` означает `ModRM.reg = n`; `ModRM.mod = 3` отвергается matcher как register form. Префиксы могут предшествовать сигнатуре, но offset указывает начало **самой сигнатуры**, а не обязательно начало инструкции.

## Коды завершения

| Код | Семантика |
| --- | --- |
| 0 | Все запрошенные inputs успешно разобраны и проверены; total_findings=0 |
| 1 | Все inputs проверены, есть хотя бы одно совпадение |
| 2 | Invalid arguments/config, I/O, malformed/unsupported input, worker failure, output failure |
| 130 | Получен SIGINT, отработала cooperative cancellation |
| 143 | Получен SIGTERM, отработала cooperative cancellation |

Ошибка важнее policy violation. Один missing file среди clean/violating files не может дать `0` или `1`. SIGINT/SIGTERM имеют приоритет в финальном коде; SIGKILL завершает процесс внешним образом, без гарантий cleanup.

## Потоки

- **stdout:** только выбранный report, help, config или каталог правил.
- **stderr:** `level=... event=...` прогресс и ошибки, без дампа environment или содержимого бинарника.
- **`--quiet`:** убирает progress, не скрывает hard command errors.
- **`--output FILE`:** report идёт только в новый файл; stdout пуст. Существующий или появившийся конкурентно destination не заменяется.

Не делайте `omsk scan artifact.so > artifact.so`: shell обрежет input **до запуска программы**. Используйте другой report path или `--output`.

## JSON schema v1

Формальное описание: [report.schema.json](report.schema.json).

Полный ожидаемый пример для `fixtures/violation.elf` (вручную составлен по fixture contract, не выдан за результат запуска):

```json
{
  "schema_version": 1,
  "tool": {"name": "omsk", "version": "0.2.0"},
  "analysis": "byte-pattern-lint",
  "input_format": "auto",
  "policy": {
    "name": "strict",
    "rules": ["syscall", "sysenter", "int80", "wrpkru", "xrstor", "xrstors", "rdtsc", "rdtscp"]
  },
  "files": [{
    "path": "fixtures/violation.elf",
    "status": "violations",
    "kind": "elf64-shared",
    "file_bytes": 132,
    "scanned_bytes": 4,
    "region_count": 1,
    "total_findings": 1,
    "omitted_findings": 0,
    "findings": [{
      "rule_id": "OMSK001",
      "rule": "syscall",
      "file_offset": 129,
      "byte_length": 2,
      "virtual_address": "0x400081",
      "region": "segment[0]"
    }]
  }],
  "summary": {
    "requested": 1, "completed": 1, "clean": 0, "violating": 1,
    "errors": 0, "cancelled": 0, "total_findings": 1, "reported_findings": 1
  },
  "complete": true,
  "exit_code": 1,
  "engine_error": null
}
```

`complete=true` означает, что **все запрошенные файлы успешно проверены**, а не «совпадений нет». `summary.completed` — число выданных per-file outcomes, включая error/cancelled; успешных проверок — `clean + violating`.

При per-file ошибке объект содержит `path`, `status` (`error`/`cancelled`) и `error`, без вымышленных findings. `engine_error` относится к инфраструктуре pipeline.

`total_findings` — полный счётчик; `findings` — ограниченный список. `omitted_findings` всегда равно `total_findings - len(findings)` для завершённого scan. Тот же принцип действует для summary.

`file_offset` — zero-based byte offset в исходном файле. `byte_length` — 2 или 3 байта сигнатуры. `virtual_address` — hex string или `null`, чтобы не терять 64-bit precision в JavaScript; у raw/ET_REL load address неизвестен.

## SARIF 2.1.0

- Стабильные rule IDs, severity `error`, kind `fail`.
- Binary location использует `region.byteOffset` и `byteLength`, не source line.
- URI содержит percent-encoded path; для absolute Linux paths используется `file://`.
- `artifacts` включает summaries всех обработанных файлов, в том числе clean/errors.
- Неполный scan даёт `invocations.executionSuccessful=false` и notification.
- Лимит списка findings даёт warning notification и сохраняет полный count.

Не все code-scanning UI одинаково отображают binary byte regions; проверьте ваш SARIF consumer. Совместимость с GitHub Code Scanning upload в этой среде не тестировалась. Plain JSON остаётся каноническим интеграционным интерфейсом для собственного CI.

## Rust API: scanner

```rust
use gulag::{scan, InputFormat, Policy, ScanOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = [0x90, 0x0f, 0x05, 0xc3];
    let report = scan(
        &bytes,
        InputFormat::Raw,
        &Policy::from_profile("syscalls")?,
        ScanOptions { max_findings: 100 },
        || false,
    )?;
    assert_eq!(report.total_findings, 1);
    assert_eq!(report.findings[0].file_offset, 1);
    Ok(())
}
```

`Policy::from_profile/from_csv/new` возвращают `Result<Policy, String>`. В собственном API можно явно преобразовать строковую ошибку в ваш error type. `ScanError` реализует `std::error::Error`. Callback отмены имеет тип `Fn() -> bool`, должен быть дешёвым, без побочных эффектов и без panic.

`parse_image(&[u8], InputFormat)` возвращает `Image { kind, regions }`. Сам parser ничего не исполняет. Для embedding безопаснее использовать `gulag`, а не устанавливать process-global signal handlers хоста.

## Rust API: channel

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut producer, mut consumer) = synapse::bounded(4)?;
    producer.send(String::from("artifact.so"))?;
    drop(producer);
    assert_eq!(consumer.recv()?, "artifact.so");
    assert!(consumer.recv().is_err());
    Ok(())
}
```

`Producer`: `send`, `try_send`. `Consumer`: `recv`, `recv_timeout`, `try_recv`. API не предоставляет clone endpoints или межпроцессную память. Повторное использование независимых jobs требует новых каналов и собственного cancellation token.
