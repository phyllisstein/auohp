#!/usr/bin/env fish

sudo chown -R $USER:$USER /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

sudo /opt/conda/bin/conda init --system fish bash zsh

/opt/conda/bin/conda init fish bash zsh
mkdir -p ~/.config/fish && touch ~/.config/fish/config.fish
echo "set -gx PATH /usr/local/cuda/bin \$PATH" | tee -a ~/.config/fish/config.fish
source ~/.config/fish/config.fish

sudo -u auohp /opt/conda/bin/conda init fish bash zsh
sudo mkdir -p /home/auohp/.config/fish && sudo touch /home/auohp/.config/fish/config.fish
echo "set -gx PATH /usr/local/cuda/bin \$PATH" | sudo tee -a /home/auohp/.config/fish/config.fish

conda activate base
pip install git+https://github.com/m-bain/whisperx.git
pip install click poetry
conda install -y pytorch torchaudio pytorch-cuda==11.8 numpy=1 spacy -c pytorch -c nvidia -c conda-forge

cd /opt/auohp/packages/scripts/src/subwhisp
poetry config virtualenvs.create false
poetry install


if python -c "import torch; assert torch.cuda.is_available(), 'CUDA not available'"
    echo "CUDA is available"
else
    echo "CUDA is not available"
    exit 1
end

subwhisp models
