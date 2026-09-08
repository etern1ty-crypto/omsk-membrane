# Реестр реализации 0.2.0

Этот файл заменяет aspirational task list исходного VM prototype. Здесь фиксируется реализованный scope, а не инструкции для автоматического исполнения.

## Внесено в исходники

- [x] Ниша: offline native artifact policy lint, без исполнения guest.
- [x] Восемь сигнатур, profiles и structured findings.
- [x] ELF64 parser и explicit raw input.
- [x] Настоящие bounded queues и конечный worker pipeline.
- [x] File/config validation, bounded I/O и report escaping.
- [x] Atomic output/no-clobber и cooperative signal shutdown.
- [x] Regression suites, fixtures, CI, README и docs.
- [x] Удалены ненужные registry dependencies и build artifacts.

## Release gates ещё не подтверждены

- [ ] Rust compilation / formatter / Clippy / rustdoc.
- [ ] Выполнение Rust tests и black-box tests скомпилированного CLI.
- [ ] Docker build/run и целевая Linux signal validation.
- [ ] Независимое security review и пилот на реальных release artifacts.

MPK/CET, executable Guest, snapshots, io_uring и lock-free IPC исключены из scope этого продукта, а не оставлены success-заглушками. Причина: [Product Discovery](docs/PRODUCT_DISCOVERY.md).
