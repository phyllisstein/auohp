#!/usr/bin/env fish

mkdir -p ~/.ssh
echo $GIT_SSH_KEY_BASE64 | base64 -d >~/.ssh/id_rsa
echo $AUOHP_PUBLIC_KEY >>~/.ssh/authorized_keys
ssh-keyscan github.com >>~/.ssh/known_hosts
chmod 700 ~/.ssh
chmod 600 ~/.ssh/id_rsa

sudo adduser --disabled-password --gecos "" auohp
sudo usermod -aG docker,sudo auohp
sudo usermod --shell /usr/bin/fish auohp

echo "%sudo ALL=(ALL) NOPASSWD:ALL" | sudo tee /etc/sudoers.d/90-sudo-group
sudo chmod 440 /etc/sudoers.d/90-sudo-group

sudo mkdir -p /home/auohp/.ssh
echo $GIT_SSH_KEY_BASE64 | base64 -d | sudo tee /home/auohp/.ssh/id_rsa
echo $AUOHP_PUBLIC_KEY | sudo tee /home/auohp/.ssh/authorized_keys
sudo ssh-keyscan github.com | sudo tee /home/auohp/.ssh/known_hosts
sudo chmod -R 600 /home/auohp/.ssh
sudo chmod 700 /home/auohp/.ssh
