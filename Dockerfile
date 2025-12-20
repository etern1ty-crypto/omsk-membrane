# ИСПОЛЬЗУЕМ БАЗОВЫЙ СЛОЙ
FROM rust:slim-bookworm

# УСТАНОВКА ИНСТРУМЕНТОВ "THE SILICA"
# Clang и LLVM нужны для генерации привязок (bindgen) и анализа.
RUN apt-get update && apt-get install -y \
    build-essential \
    clang \
    llvm-dev \
    libclang-dev \
    pkg-config \
    git \
    && rm -rf /var/lib/apt/lists/*

# АКТИВАЦИЯ NIGHTLY-РЕЖИМА
# OMSK требует доступа к нестабильным функциям CPU.
RUN rustup toolchain install nightly && \
    rustup default nightly && \
    rustup component add rust-src

# ПОДГОТОВКА РАБОЧЕЙ ЗОНЫ
WORKDIR /usr/src/omsk
COPY . .

# КОМПИЛЯЦИЯ
# Флаг --release обязателен. Debug-сборки недопустимы для Production.
CMD ["cargo", "build", "--release"]