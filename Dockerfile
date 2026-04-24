# syntax=docker/dockerfile:1.7-labs

# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ Watchman Binaries ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ #
FROM phyllisstein/watchman:v2025.07.21.00 AS watchman

# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ App ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ #
FROM ubuntu:24.04 AS app

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY --from=watchman /usr/local/bin/* /usr/local/bin/
COPY --from=watchman /usr/local/lib/* /usr/local/lib/

ENV CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/app/target \
    MODELS_DIR=/models \
    NODE_MAJOR=24 \
    PATH="/app/node_modules/.bin:/usr/share/nodejs/yarn/bin:/usr/local/cargo/bin:$PATH" \
    PROJECT_PATH=/app \
    RUSTFLAGS="-C target-feature=+fp16" \
    RUSTUP_HOME=/usr/local/rustup

RUN mkdir -p ${CARGO_TARGET_DIR} ${CARGO_HOME} ${RUSTUP_HOME} ${MODELS_DIR} /usr/local/var/run/watchman \
    && chmod a+w ${CARGO_TARGET_DIR} ${CARGO_HOME} ${RUSTUP_HOME} ${MODELS_DIR} /usr/local/var/run/watchman \
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
    && curl -sL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor | tee /usr/share/keyrings/yarnkey.gpg >/dev/null \
    && echo "deb [signed-by=/usr/share/keyrings/yarnkey.gpg] https://dl.yarnpkg.com/debian stable main" | tee /etc/apt/sources.list.d/yarn.list \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages \
        nodejs \
        yarn \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile default -y \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

WORKDIR /app

COPY --parents ./scripts ./

ENTRYPOINT ["./scripts/develop.sh"]

CMD ["watch"]
