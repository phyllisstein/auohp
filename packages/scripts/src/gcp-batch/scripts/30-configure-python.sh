#!/usr/bin/env zsh

set -eEuxo pipefail

sudo chown -R auohp:auohp /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init fish bash zsh
. /home/auohp/.zshrc
/opt/conda/bin/conda activate base

python install_dependencies.py

poetry config virtualenvs.create false
poetry install
