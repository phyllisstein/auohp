#!/usr/bin/env bash

set -eEuxo pipefail

sudo usermod -aG docker,sudo auohp

echo "%sudo ALL=(ALL) NOPASSWD:ALL" | sudo tee /etc/sudoers.d/90-sudo-group
sudo chmod 440 /etc/sudoers.d/90-sudo-group
