#!/usr/bin/env fish

sudo mkdir -p /opt/auohp
sudo chown $USER:$USER /opt/auohp
git clone $AUOHP_GIT_REPO_URL /opt/auohp
