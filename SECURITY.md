# Security policy

## Не использовать как sandbox

OMSK Membrane — статический byte-pattern lint. Чистый отчёт не разрешает исполнение недоверенного native code. Полная [модель угроз](docs/THREAT_MODEL.md) является частью публичного контракта.

## Reporting

Сообщайте о возможных parser crashes, unsafe-ошибках, silently passing failures, report injection, небезопасной записи и проблемах shutdown через приватный канал владельца вашего fork. Если в GitHub-репозитории включён Private vulnerability reporting, используйте его. Контактные данные и SLA здесь не выдумываются.

Не публикуйте реальные клиентские бинарники, секреты и чувствительные пути в публичном issue. По возможности подготовьте минимальный синтетический fixture, Rust/toolchain/OS версии, expected/actual behavior и точную команду воспроизведения.

## Supported versions

В этой выдаче только кандидат 0.2.0. Он не прошёл compiler/runtime validation в среде подготовки. До публикации подтверждённого релиза нет обещания security support SLA. Потребители должны закреплять проверенный source commit и проходить собственные release gates.

## Supply chain

Cargo.lock содержит только локальные crates. Это не означает отсутствие platform vulnerabilities: Rust compiler/std, OS libc, base images и CI actions проверяются отдельно. Перед production обновите и зафиксируйте одобренные версии/digests, выполните container/toolchain scanning.

Исходная memmap2 0.9.9 была в affected range [RUSTSEC-2026-0186](https://rustsec.org/advisories/RUSTSEC-2026-0186.html); зависимость удалена. Reachable exploit из предоставленного исходника не установлен.
