# Развёртывание и Production

[← README](../README.md)

## Release gate: сначала подтверждение сборки

Эта выдача не прошла Rust compilation в среде подготовки: Cargo/rustc отсутствовали, сеть для установки была заблокирована. **Не внедряйте blocking production gate, пока не прошли следующие проверки.**

```bash
bash scripts/verify.sh
```

Скрипт сначала запускает formatter (изменения нужно включить в первый commit), затем Clippy, Rust tests, rustdoc, debug/release build и black-box tests обоих binaries. Автоматизация есть в [CI workflow](../.github/workflows/ci.yml). Наличие workflow не означает, что он уже выполнялся.

Перед внешним релизом дополнительно:

- проверить signal FFI и parser независимым ревью;
- подтвердить работу на вашем Linux/kernel/filesystem и реальных immutable artifacts;
- обновить baseline toolchain до одобренного вашей security policy stable и зафиксировать точную версию;
- зафиксировать base image digests, версии CI actions и проверить OS/standard-library advisories;
- проверить потребитель SARIF, false positives и ресурсные лимиты;
- сформировать build manifest с hashes и подтверждёнными test logs.

`1.85.1` в toolchain/Docker — воспроизводимая исходная baseline, не утверждение, что это новейший безопасный Rust на дату релиза. CI также проверяет `stable`. Проект не содержит registry crates, но compiler, std, libc OS, container image и CI actions остаются частью supply chain.

## Локальная установка

```bash
cargo build --offline --locked --release -p reactor --bin omsk
install -m 0755 target/release/omsk "$HOME/.local/bin/omsk"
omsk --version
```

Каталог `$HOME/.local/bin` должен существовать и быть в PATH. Если нет, используйте `mkdir -p "$HOME/.local/bin"` и полный путь к бинарнику. Root не нужен.

Offline означает отсутствие загрузок **Cargo crates** при уже установленном toolchain. Первоначальная установка Rust/components, Docker base images и CI actions требует сети или заранее подготовленного internal mirror/cache.

## Container

Dockerfile — multi-stage build; runtime непривилегированный UID/GID 10001, без compiler и package build tools. Он запускает CLI, а не сборку по CMD.

```bash
docker build --tag omsk-membrane:0.2.0 .
docker run --rm --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges --memory 256m --cpus 1 \
  -v "$PWD/fixtures:/work:ro" \
  omsk-membrane:0.2.0 scan --format json clean.elf
```

Сохранение stdout удобно делать на хосте. Не пробрасывайте Docker socket, секреты, HOME или весь filesystem. Увеличивайте memory limit только после измерения нагрузок. Container build/runtime здесь **не выполнялись** — Docker daemon отсутствует.

Для `--output` нужен отдельный writable каталог, принадлежащий UID 10001, с поддержкой hard links. Input volume оставляйте read-only. Обработка `SIGTERM` рассчитана на прямой запуск бинарника через exec-form ENTRYPOINT; shell wrapper должен использовать `exec`.

## Пример CI gate без потери exit code

```bash
set +e
./target/release/omsk scan --config .env.example --format json \
  --output "omsk-${CI_JOB_ID:-local}.json" fixtures/clean.elf fixtures/violation.elf
status=$?
set -e
case "$status" in
  0) echo "No selected byte patterns found" ;;
  1) echo "Policy matches: review report" >&2 ;;
  2) echo "Scanner error: gate is not passing" >&2 ;;
  130|143) echo "Scanner interrupted" >&2 ;;
  *) echo "Unexpected process failure: $status" >&2 ;;
esac
exit "$status"
```

Это пример с fixtures; для настоящего pipeline замените их явными release artifacts. Не используйте `|| true` на blocking gate. Если поначалу нужен advisory rollout, настройте необязательный job в CI, сохраняя исходный код результата и report; не подменяйте errors на pass.

## Ресурсы и время

- По умолчанию файл до 64 MiB; максимальная настройка 1 GiB.
- Один scanner worker; max 512 input paths.
- Job queue 4, result queue 2, max stored findings 1000 на файл.
- Cancellation checkpoints не являются hard timeout.

Запускайте на локальном immutable storage. На NFS/FUSE даже чтение regular file может зависнуть внутри kernel; внешний CI timeout/container limit остаётся необходимым. CPU/time benchmarks не выполнялись; прежние 5M OPS и <2% overhead не перенесены.

## Наблюдаемость и инциденты

Собирайте stdout-report и stderr отдельно. Progress events: `scan_start`, `file_complete`, `scan_end`, `command_failed`. Path/error strings экранируются. Содержимое артефактов и environment не логируется.

При `complete=false` проверьте per-file errors, `engine_error`, signal exit и filesystem. При `--output` already exists выберите новый путь, не удаляйте отчёт автоматически. При отказе fsync после публикации файл может быть уже создан — сначала проверьте его.

Rollback: убрать обязательность отдельного scan job, не отключая остальные security checks. OMSK не модифицирует inputs и не меняет глобальные host security settings.
