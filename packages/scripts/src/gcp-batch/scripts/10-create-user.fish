#!/usr/bin/env fish

sudo adduser --disabled-password --gecos "" auohp
sudo usermod -aG docker,sudo auohp
sudo usermod --shell /usr/bin/fish auohp
sudo usermod --shell /usr/bin/fish $USER

echo "%sudo ALL=(ALL) NOPASSWD:ALL" | sudo tee /etc/sudoers.d/90-sudo-group
sudo chmod 440 /etc/sudoers.d/90-sudo-group
