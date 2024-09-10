#!/usr/bin/env fish

adduser --disabled-password --gecos "" auohp
usermod -aG docker,sudo auohp
usermod --shell /usr/bin/fish auohp
usermod --shell /usr/bin/fish $USER

echo "%sudo ALL=(ALL) NOPASSWD:ALL" | tee /etc/sudoers.d/90-sudo-group
sudo chmod 440 /etc/sudoers.d/90-sudo-group
