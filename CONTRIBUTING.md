# Contributing

Спасибо за вклад. Главный принцип проекта: измеряемое поведение важнее красивых заявлений о безопасности.

## Подготовка

64-bit Linux, Rust 1.85+ с rustfmt/Clippy, Python 3.11+. Точная baseline указана в `rust-toolchain.toml`. Не добавляйте registry dependencies без обоснования функциональности, лицензии, maintenance и advisory review.

```bash
bash scripts/verify.sh
```

Скрипт форматирует исходники и запускает реальные проверки; сохраните форматирующие изменения в commit. При отсутствии compiler он завершается ошибкой. Не меняйте badge на passing только потому, что YAML workflow существует.

## Изменение правила

1. Стабильный ID и имя в `gulag/src/rules.rs`.
2. Документируйте точную byte signature, ModRM/prefix semantics и known false positives.
3. Тесты на offsets, truncated suffixes, разрешённые соседние encodings и policy selection.
4. Обновите CLI reference, schema enum, fixtures и catalogue assertions.
5. Не называйте матч доказанной reachable instruction, если decoding не реализован.

## Изменение parser

Все поля и сложения checked до slice indexing. Unsupported input — error, не raw fallback и не clean. Добавьте malformed/boundary/regression fixtures. Не исполняйте входные бинарники в тестах. Новые formats требуют полноценного parsing contract, а не success stub.

## Изменение runtime

Ни один output/worker error не должен давать exit 0. Проверяйте backpressure, drop ordering, malformed files, signal cancellation и atomic output race. Не расширяйте unsafe за пределы изолированного и документированного модуля без отдельного ревью.

## Pull request

Опишите проблему, user-visible change, совместимость API/schema и реально выполненные команды. Отделяйте tests authored от tests passed. Не публикуйте customer artifacts, credentials или private paths без согласия владельца.

Security issues: [SECURITY.md](SECURITY.md). Модель угроз: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
