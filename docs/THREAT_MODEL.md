# Модель угроз и ограничения

[← README](../README.md)

## Что именно проверяется

Выбранные byte signatures отсутствуют или присутствуют в определённых file-backed executable regions файла, прочитанного в immutable in-process buffer. Формат/ввод с ошибкой не превращается в passing scan.

OMSK **не исполняет** проверяемый файл. Внутренний Rust worker не является guest sandbox. В файле `guest.rs` нет виртуальной машины — это переиспользованное имя модуля bounded reader.

## Чего результат не доказывает

- Безопасности запуска, memory safety, code integrity или отсутствия вредоносной логики.
- Отсутствия всех способов syscall, изменения памяти или side channels.
- Валидных границ инструкций, достижимости control flow и полноты disassembly.
- Детерминизма при профиле `timing`.
- Безопасности shared libraries, loader, relocations, JIT/self-modifying code.
- Реализации MPK, CET, shadow stacks, snapshots, io_uring или zero-syscall runtime.
- Hard wall-clock/memory guarantee на shared/NFS/FUSE storage.

Даже полноценный дизассемблер без runtime isolation не решил бы эти задачи. [Исследование Native Client](https://research.google.com/pubs/archive/34913.pdf) описывает дополнительные code-layout, control-flow и memory constraints для sandboxing.

## False positives и false negatives

Поиск консервативен на уровне **байтов**. `MOV eax, 0x0000050f` содержит последовательность `0F 05`, но это не инструкция SYSCALL. Такая находка намеренно видна; не называйте её доказанной уязвимостью.

ELF selection исключает неисполняемые области, однако executable regions могут содержать константы. Сигнатура может начинаться внутри другой инструкции. Preceding prefixes не входят в `file_offset`/`byte_length`.

Обратное ограничение: чистый результат не исключает опасные инструкции вне текущего набора, привнесённый loader/runtime code, вызов опасного кода через импорт, генерацию инструкций во время исполнения и изменение прав страниц. Метаданные ELF используются для определения предмета lint, не являются доверенной аттестацией того, что OS в действительности исполнит именно эти байты.

ET_REL анализируется до линковки; отдельные sections независимы. Финальная линковка/relocations могут менять код — release gate следует запускать также на окончательном immutable `.so`.

## Враждебный файл

Для уменьшения attack surface:

- только explicit supported formats; no auto raw fallback;
- checked ranges/overflow, ограниченное количество таблиц;
- отказ от overlapping/unsupported executable layouts;
- файл ограничен по размеру до и во время чтения;
- findings storage ограничен; полный счётчик не теряется;
- reader использует owned buffer, а не file-backed mmap, изменяемый извне;
- labels формируются из числовых индексов и имеют ограниченный размер;
- JSON и terminal output экранируются;
- queues ограничены и lifecycle конечен.

Это инженерные меры, но не сертификат отсутствия parser bugs. Для недоверенных artifacts запускайте сам scanner в отдельном ограниченном CI container/VM с immutable read-only input. Не давайте ему secrets и сетевой доступ.

## Файловая система и отчёты

Путь input должен указывать на regular file. Linux O_NOFOLLOW защищает последний компонент от symlink swap; O_NONBLOCK не позволяет зависнуть на FIFO, подменённом перед open. Повторная проверка inode/dev, размера и mtime выявляет часть concurrent modifications.

Это не криптографическая гарантия snapshot consistency. Атакующий, меняющий content с восстановлением mtime/size, может обойти detection изменения; hard links и symlinks в parent directories не являются containment-политикой. Input workspace должен быть immutable и доверенно управляться runner.

Output parent должен принадлежать доверенному процессу и поддерживать hard links. Temp создаётся через create_new с mode 0600, publication не заменяет существующий destination. SIGKILL может оставить temp; они не должны считаться готовыми reports. Crash/power loss не гарантируют cleanup.

## Signal FFI

Единственный unsafe-модуль — `reactor/src/shutdown.rs`, 64-bit Linux libc ABI для `signal`. Handler имеет static lifetime и выполняет только AtomicI32 store/compare-exchange. Guard сохраняет старые dispositions и восстанавливает после join.

Это требует независимого ревью и реальных SIGINT/SIGTERM tests перед production. На неподдерживаемой платформе scan возвращает ошибку, а не симулирует graceful shutdown. Встраиваемое приложение должно управлять своими signal handlers; используйте `gulag` напрямую.

## За пределами scope

PE/Mach-O/ELF32/big-endian/non-AMD64, extended ELF numbering, executable zero-fill tails и compressed executable sections сейчас явно не поддержаны. Для PE/Mach-O нет заглушек, которые бы возвращали «успех».

Механизм исключений по regex/offset, криптографическая подпись, hash attestation, multi-process IPC, SaaS и policy management не реализованы и не заявляются. Не добавляйте широкие подавления для получения зелёного CI без review.
