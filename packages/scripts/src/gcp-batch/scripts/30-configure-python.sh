#!/usr/bin/env bash

set -eEuxo pipefail

sudo chown -R auohp:auohp /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init bash
. /home/auohp/.bashrc
/opt/conda/bin/conda activate base

python install_dependencies.py

poetry config virtualenvs.create false
poetry install
