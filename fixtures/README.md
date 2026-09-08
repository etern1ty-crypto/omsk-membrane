# Scanner fixtures — DO NOT EXECUTE

Эти файлы предназначены для чтения, разбора и дизассемблирования, не для запуска. «clean» означает отсутствие выбранных сигнатур, не безопасность кода.

- `clean.bin`, `syscall.bin`, `all-rules.bin`: minimal raw code bytes.
- `immediate.bin`: MOV immediate содержит 0F 05; демонстрирует намеренную byte-level консервативность.
- `clean.elf`, `violation.elf`: минимальные ELF64 AMD64 с одним executable PT_LOAD, без section table.
- `data-only-pattern.elf`: сигнатура за пределами executable segment не сканируется.
- `nonexec.elf`: отсутствие executable regions должно дать ошибку.
- `violation.o`: ELF64 ET_REL с executable PROGBITS.
- `truncated.elf`, `empty.bin`, `unknown.bin`: негативные cases.

[manifest.json](manifest.json) задаёт размеры, SHA-256 и ожидаемые результаты. [Генератор](../scripts/make_fixtures.py) детерминирован и использует только Python stdlib.
