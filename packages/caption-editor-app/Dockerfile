FROM phyllisstein/watchman:v2022.06.20 AS watchman

# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ App ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ #
FROM ubuntu:20.04 AS app

COPY --from=watchman /usr/local/bin/* /usr/local/bin/
COPY --from=watchman /usr/local/lib/* /usr/local/lib/

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages --no-install-recommends \
    ca-certificates \
    libboost-context1.71.0 \
    libevent-2.1-7 \
    libsnappy1v5 \
    curl \
    git \
    netcat \
  && curl -fsSL https://deb.nodesource.com/setup_18.x | bash - \
  && curl -sL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor | tee /usr/share/keyrings/yarnkey.gpg >/dev/null \
  && echo "deb [signed-by=/usr/share/keyrings/yarnkey.gpg] https://dl.yarnpkg.com/debian stable main" | tee /etc/apt/sources.list.d/yarn.list \
  && apt-get update \
  && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages \
    nodejs \
    yarn \
  && mkdir -p /usr/local/var/run/watchman

ENV PATH="/app/node_modules/.bin:/usr/share/nodejs/yarn/bin:$PATH"

WORKDIR /app

COPY scripts/develop.sh ./scripts/develop.sh
COPY scripts/watchman ./scripts/watchman

RUN ./scripts/develop.sh watches

CMD ["./scripts/develop.sh", "watch"]
