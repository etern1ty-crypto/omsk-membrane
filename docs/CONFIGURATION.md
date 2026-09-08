# Настройка и конфигурация

[← README](../README.md)

## Приоритет

`defaults < --config FILE < OMSK_* environment < CLI`

`.env` не читается автоматически. Команда `omsk config` печатает [пример](../.env.example), но сама не создаёт файл. `--config` принимает regular UTF-8 file не более 16 KiB.

```bash
./target/release/omsk scan --config .env.example --format json fixtures/clean.elf
```

Параметры проверяются после применения слоёв. Неизвестные ключи и дубликаты в config-файле — ошибки, даже если последующий слой мог бы их перекрыть. Неизвестные process environment keys игнорируются. Известная env-переменная с не-UTF-8 значением — ошибка.

Особое правило профилей: явный профиль более высокого уровня сбрасывает custom `OMSK_DENY` нижнего уровня. Если в одном уровне заданы и профиль, и список deny, список заменяет профиль. Порядок разных опций внутри одного CLI-вызова это не меняет.

## Полный справочник

| Env | CLI | Default | Допустимые значения |
| --- | --- | --- | --- |
| `OMSK_PROFILE` | `--profile` | `strict` | `strict`, `syscalls`, `timing` |
| `OMSK_DENY` | `--deny` | отсутствует | Непустой CSV из известных имён правил; без дубликатов |
| `OMSK_INPUT_FORMAT` | `--input-format` | `auto` | `auto`, `elf`, `raw` |
| `OMSK_FORMAT` | `--format` | `text` | `text`, `json`, `sarif` |
| `OMSK_MAX_FILE_BYTES` | `--max-file-bytes` | `67108864` (64 MiB) | Decimal integer `1..1073741824` |
| `OMSK_MAX_FINDINGS` | `--max-findings` | `1000` | Decimal integer `1..100000` на файл |
| `OMSK_QUEUE_CAPACITY` | `--queue-capacity` | `4` | Decimal integer `1..64` |
| `OMSK_QUIET` | `--quiet` / `--no-quiet` | `false` | `true`, `false` |

Нет сокращений `64M`, знаков `+/-`, shell expansion или числа с подчёркиваниями. Пустой deny не означает «разрешить всё»: он отвергается. Empty/unsupported artifacts тоже отвергаются.

`--config`, `--output` и пути к входным файлам намеренно задаются только в CLI. Environment не может незаметно направить запись в произвольный файл.

## Профили

| Профиль | Правила |
| --- | --- |
| `syscalls` | `syscall`, `sysenter`, `int80` |
| `timing` | `rdtsc`, `rdtscp` |
| `strict` | Все восемь правил |

`timing` не доказывает детерминизм: другие источники времени, random instructions, shared memory и OS calls этим профилем не покрываются. `strict` — максимально широкий **текущий** набор сигнатур, не complete ISA security policy.

```bash
OMSK_PROFILE=timing ./target/release/omsk scan --input-format raw fixtures/all-rules.bin
./target/release/omsk scan --deny syscall,sysenter,int80 --input-format raw fixtures/all-rules.bin
```

Ожидаемые counts — 2 и 3 соответственно; обе команды должны вернуть `1`. Это ожидаемые значения fixtures, а не утверждение о выполнении Rust-команд в среде подготовки.

## Синтаксис env-файла

- `KEY=value`, blank lines, комментарии `#`.
- CRLF и UTF-8 BOM поддерживаются.
- Значение можно целиком заключить в `'` или `"`; после закрывающей кавычки допустим комментарий.
- Escapes, `$VARIABLE`, command substitution, `export`, backticks и multiline values не поддерживаются.
- Парсер не запускает shell и не выполняет команды.

```dotenv
OMSK_PROFILE=syscalls
OMSK_FORMAT="sarif" # машинный отчёт
OMSK_MAX_FILE_BYTES=33554432
OMSK_MAX_FINDINGS=500
OMSK_QUEUE_CAPACITY=2
OMSK_QUIET=true
```

## Пути и границы

До 512 уникальных **буквально разных** аргументов-путей, каждый 1..4096 UTF-8 bytes. Не выполняются recursion, glob expansion, чтение stdin или поиск на диске. Glob может развернуть shell — его результат всё равно проходит лимит аргументов. `-` означает буквальное имя файла, не stdin.

```bash
./target/release/omsk scan -- --plugin.so
```

CLI принимает только UTF-8 аргументы. Symlink в последнем компоненте input path запрещён; symlink в родительском каталоге не является отдельной политикой containment. Hard links и различные алиасы пути не дедуплицируются по inode. Используйте immutable CI workspace.

## Что фиксировать в production

Храните policy config рядом с build recipe, запускайте с явным `--config` и очищайте конфликтующие `OMSK_*` в runner environment. Записывайте версию scanner, конфигурацию, build identity и SHA-256 артефакта отдельно в release manifest. CLI не предоставляет криптографической аттестации input bytes.
