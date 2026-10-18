# syntax=docker/dockerfile:1.7-labs

# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ Watchman Binaries ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ #
FROM phyllisstein/watchman:v2026.07.27.00 AS watchman

# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ App ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ #
FROM ubuntu:26.04 AS app

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY --from=watchman /usr/local/bin/* /usr/local/bin/
COPY --from=watchman /usr/local/lib/* /usr/local/lib/

ENV CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/target \
    NODE_MAJOR=26 \
    PATH="/app/node_modules/.bin:/usr/share/nodejs/yarn/bin:/usr/local/cargo/bin:/usr/local/pnpm/bin:$PATH" \
    # PNPM_CONFIG_NODE_LINKER=hoisted \
    PNPM_HOME=/usr/local/pnpm \
    PROJECT_PATH=/app \
    RUSTFLAGS="-C target-feature=+fp16" \
    RUSTUP_HOME=/usr/local/rustup

RUN mkdir -p ${CARGO_TARGET_DIR} ${CARGO_HOME} ${RUSTUP_HOME} /usr/local/var/run/watchman \
    && chmod a+w ${CARGO_TARGET_DIR} ${CARGO_HOME} ${RUSTUP_HOME} /usr/local/var/run/watchman \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        curl \
        git \
        gnupg \
        libclang-dev \
        libssl-dev \
        pkg-config \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
        | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_MAJOR}.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list >/dev/null \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages \
        nodejs \
    && curl -fsSL https://get.pnpm.io | SHELL=bash sh - \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile default --default-toolchain none -y \
    && curl -Ls https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin UV_NO_MODIFY_PATH=1 sh \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

WORKDIR /app

COPY rust-toolchain.toml ./
RUN rustup toolchain install

COPY --parents ./scripts ./

ENTRYPOINT ["./scripts/watchman/develop.sh"]

CMD ["watch"]
