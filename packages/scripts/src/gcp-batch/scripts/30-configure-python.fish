#!/usr/bin/env fish

sudo chown -R $USER:$USER /opt/conda
cd /opt/auohp/packages/scripts/src/subwhisp

/opt/conda/bin/conda init fish bash zsh
sudo /opt/conda/bin/conda init --system fish bash zsh
mkdir -p ~/.config/fish && touch ~/.config/fish/config.fish
echo "set -gx PATH /usr/local/cuda/bin \$PATH" | tee -a ~/.config/fish/config.fish
source ~/.config/fish/config.fish

sudo -u auohp /opt/conda/bin/conda init fish bash zsh
sudo mkdir -p /home/auohp/.config/fish && sudo touch /home/auohp/.config/fish/config.fish
echo "set -gx PATH /usr/local/cuda/bin \$PATH" | sudo tee -a /home/auohp/.config/fish/config.fish

cd /opt/auohp/packages/scripts/src/subwhisp
conda env update -n base -f environment.yml
# conda install -y pytorch-cuda=11.3 torchvision torchaudio cudatoolkit -c pytorch -c nvidia
conda install -y pytorch-cuda=12 torchvision torchaudio cudatoolkit pytorch -c pytorch -c nvidia

poetry config virtualenvs.create false
poetry install

subwhisp models
