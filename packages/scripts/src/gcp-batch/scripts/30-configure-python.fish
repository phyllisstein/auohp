#!/usr/bin/env fish

sudo chown -R $USER:$USER /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init fish bash zsh
sudo -u auohp /opt/conda/bin/conda init fish bash zsh
sudo /opt/conda/bin/conda init --system fish bash zsh
mkdir -p ~/.config/fish && touch ~/.config/fish/config.fish
sudo mkdir -p /home/auohp/.config/fish && sudo touch /home/auohp/.config/fish/config.fish
echo "set -gx PATH /usr/local/cuda-12.3/bin \$PATH" | sudo tee -a /home/auohp/.config/fish/config.fish
source ~/.config/fish/config.fish
set -gx PATH /usr/local/cuda-12.3/bin $PATH

cd /opt/auohp/packages/scripts/src/subwhisp
conda env update -n base -f environment.yml
conda install -y pytorch=2.3 torchvision torchaudio cudatoolkit -c pytorch -c nvidia

poetry config virtualenvs.create false
poetry install

if python -c "import torch; assert torch.cuda.is_available(), 'CUDA not available'"
    echo "CUDA is available"
else
    echo "CUDA is not available"
    exit 1
end

subwhisp models
