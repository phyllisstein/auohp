FROM ubuntu:noble

ENV NODE_MAJOR=20

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages --no-install-recommends \
        ca-certificates \
        curl \
        git \
        gnupg \
        less \
        lsof \
        wget \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_$NODE_MAJOR.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list \
    && curl -sL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor | tee /usr/share/keyrings/yarnkey.gpg >/dev/null \
    && echo "deb [signed-by=/usr/share/keyrings/yarnkey.gpg] https://dl.yarnpkg.com/debian stable main" | tee /etc/apt/sources.list.d/yarn.list \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages \
        nodejs \
        yarn

ENV PATH="/app/node_modules/.bin:/usr/share/nodejs/yarn/bin:$PATH"

RUN useradd -ms /bin/sh -u 1001 app
USER app

WORKDIR /app

COPY --chown=app:app package.json yarn.lock .yarnrc.yml ./
COPY --chown=app:app .yarn/releases .yarn/releases
RUN --mount=type=secret,id=environment,uid=1001,gid=1001 \
    . /run/secrets/environment \
    && export GSAP_NPM_TOKEN FONT_AWESOME_NPM_TOKEN GITHUB_TOKEN \
    && yarn install

COPY --chown=app:app . /app

CMD ["yarn", "dev"]
