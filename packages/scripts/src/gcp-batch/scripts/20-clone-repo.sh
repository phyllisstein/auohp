#!/usr/bin/env zsh

set -eEuxo pipefail

mkdir -p /home/auohp/.ssh
echo "$GIT_SSH_KEY_BASE64" | base64 -d | tee /home/auohp/.ssh/id_rsa
echo "$AUOHP_PUBLIC_KEY" | tee /home/auohp/.ssh/authorized_keys
ssh-keyscan github.com | tee /home/auohp/.ssh/known_hosts
chmod -R 600 /home/auohp/.ssh
chmod 700 /home/auohp/.ssh

sudo mkdir -p /opt/auohp
sudo chown auohp:auohp /opt/auohp
git clone "$AUOHP_GIT_REPO_URL" /opt/auohp
