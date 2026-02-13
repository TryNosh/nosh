# Benchmark container for comparing nosh vs zsh+spaceship startup time
FROM rustlang/rust:nightly-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    zsh \
    git \
    curl \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Install spaceship-prompt for zsh
RUN git clone https://github.com/spaceship-prompt/spaceship-prompt.git /root/.zsh/spaceship --depth=1

# Configure zsh with spaceship (minimal config, just spaceship)
RUN echo 'source /root/.zsh/spaceship/spaceship.zsh' > /root/.zshrc && \
    echo 'export SPACESHIP_PROMPT_ASYNC=false' >> /root/.zshrc

# Copy nosh binary
COPY --from=builder /build/target/release/nosh /usr/local/bin/nosh

# Set up nosh config directory and initialize builtins
RUN mkdir -p /root/.config/nosh

# Create a test directory with git + Cargo.toml (realistic scenario)
WORKDIR /test-project
RUN git init && \
    printf '[package]\nname = "test"\nversion = "0.1.0"' > Cargo.toml && \
    git add . && \
    git config user.email "test@test.com" && \
    git config user.name "Test" && \
    git commit -m "init"

# Initialize nosh (creates packages/builtins)
RUN nosh -c 'exit' || true

# Copy benchmark script
COPY benchmarks/time-to-prompt.py /benchmark.py
RUN chmod +x /benchmark.py

CMD ["python3", "/benchmark.py"]
