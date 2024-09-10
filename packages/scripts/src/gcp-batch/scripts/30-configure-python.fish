#!/usr/bin/env fish

sudo chown -R $USER:$USER /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init fish bash zsh
source ~/.config/fish/config.fish

python install_dependencies.py

poetry config virtualenvs.create false
poetry install
