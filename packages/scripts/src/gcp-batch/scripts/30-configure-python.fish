#!/usr/bin/env fish

sudo chown -R $USER:$USER /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init fish bash zsh
source ~/.config/fish/config.fish

python install_dependencies.py

poetry config virtualenvs.create false
poetry install

if python -c "import torch; assert torch.cuda.is_available(), 'CUDA not available'"
    echo "CUDA is available"
else
    echo "CUDA is not available"
    exit 1
end

subwhisp models
