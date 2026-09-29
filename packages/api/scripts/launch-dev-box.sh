#!/usr/bin/env bash

set -euo pipefail

# Quick-and-dirty AWS CLI bootstrap for an AUOHP GPU dev box.
#
# What this script does:
# - Resolves the current Ubuntu 24.04 Deep Learning Base AMI (CUDA/driver
#   pre-baked) via AWS SSM.
# - Launches a GPU instance.
# - Waits for the instance to come up.
# - SSHes in to install build tools and reboot once for a clean slate.
# - SSHes in again to verify the GPU, install Rust, clone/pull the repo, and
#   print the cargo command that worked for manual pipeline runs.
#
# What this script does not try to do:
# - Be generic across regions or Linux distributions.
# - Create an AMI or launch template.
# - Handle every failure mode gracefully.
# - Hide the fact that GPU driver setup is a little messy on first boot.
#
# Treat this as a reference script you can tweak, not polished infrastructure.

# Required inputs. Export these before running, or edit the defaults below.
: "${AWS_REGION:=us-east-2}"
: "${AWS_PROFILE:=default}"
: "${AWS_KEY_NAME:=}"
: "${AWS_SECURITY_GROUP_ID:=}"
: "${AWS_SUBNET_ID:=}"
: "${AWS_ACCESS_KEY_ID:=}"
: "${AWS_SECRET_ACCESS_KEY:=}"

# Optional inputs.
: "${AWS_INSTANCE_NAME:=auohp-g7-dev}"
: "${AWS_INSTANCE_TYPE:=g7.2xlarge}"
: "${AWS_DISK_SIZE_GB:=50}"
: "${AWS_SSH_USER:=ubuntu}"
: "${AWS_SSH_KEY_PATH:=}"
: "${AWS_INSTANCE_PROFILE_NAME:=}"
: "${AUOHP_REPO_URL:=https://github.com/phyllisstein/auohp.git}"
: "${AUOHP_REPO_DIR:=/home/ubuntu/auohp}"

if [[ -z "${AWS_KEY_NAME}" || -z "${AWS_SECURITY_GROUP_ID}" || -z "${AWS_SUBNET_ID}" ]]; then
    cat <<'EOF'
Missing required environment.

Set at least:
    AWS_KEY_NAME=...
    AWS_SECURITY_GROUP_ID=...
    AWS_SUBNET_ID=...
    AWS_ACCESS_KEY_ID=...
    AWS_SECRET_ACCESS_KEY=...
    AWS_REGION=...

Optional but usually useful:
    AWS_PROFILE=default
    AWS_SSH_KEY_PATH=~/.ssh/your-key.pem
EOF
    exit 1
fi

if [[ -z "${AWS_SSH_KEY_PATH}" ]]; then
    # Fall back to the conventional PEM path if the caller did not set one.
    AWS_SSH_KEY_PATH="$HOME/.ssh/${AWS_KEY_NAME}.pem"
fi

if [[ ! -f "${AWS_SSH_KEY_PATH}" ]]; then
    echo "SSH private key not found: ${AWS_SSH_KEY_PATH}" >&2
    exit 1
fi

AWS_BASE=(aws --profile "${AWS_PROFILE}" --region "${AWS_REGION}")

echo "Resolving the current Ubuntu 24.04 Deep Learning Base AMI from AWS SSM..."
AMI_ID="$(${AWS_BASE[@]} ssm get-parameter \
    --name /aws/service/deeplearning/ami/x86_64/base-with-single-cuda-ubuntu-24.04/latest/ami-id \
    --query 'Parameter.Value' \
    --output text)"

echo "Using AMI: ${AMI_ID}"

RUN_ARGS=(
    ec2 run-instances
        --block-device-mappings "[{\"DeviceName\":\"/dev/sda1\",\"Ebs\":{\"VolumeSize\":${AWS_DISK_SIZE_GB},\"VolumeType\":\"gp3\"}}]"
        --ebs-optimized
        --image-id "${AMI_ID}"
        --instance-type "${AWS_INSTANCE_TYPE}"
        --key-name "${AWS_KEY_NAME}"
        --output text
        --query 'Instances[0].InstanceId'
        --security-group-ids "${AWS_SECURITY_GROUP_ID}"
        --subnet-id "${AWS_SUBNET_ID}"
        --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=${AWS_INSTANCE_NAME}}]"
)

if [[ -n "${AWS_INSTANCE_PROFILE_NAME}" ]]; then
    RUN_ARGS+=(--iam-instance-profile "Name=${AWS_INSTANCE_PROFILE_NAME}")
fi

echo "Launching ${AWS_INSTANCE_TYPE}..."
INSTANCE_ID="$(${AWS_BASE[@]} "${RUN_ARGS[@]}")"
echo "Instance ID: ${INSTANCE_ID}"

echo "Waiting for EC2 status checks to pass..."
${AWS_BASE[@]} ec2 wait instance-status-ok --instance-ids "${INSTANCE_ID}"

PUBLIC_IP="$(${AWS_BASE[@]} ec2 describe-instances \
    --instance-ids "${INSTANCE_ID}" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' \
    --output text)"

echo "Public IP: ${PUBLIC_IP}"

# Small SSH wrapper so we keep the connection flags in one place.
ssh_box() {
    ssh \
        -o StrictHostKeyChecking=accept-new \
        -o ConnectTimeout=10 \
        -i "${AWS_SSH_KEY_PATH}" \
        "${AWS_SSH_USER}@${PUBLIC_IP}" "$@"
}

wait_for_ssh() {
    local attempt
    for attempt in $(seq 1 30); do
        if ssh_box true >/dev/null 2>&1; then
            return 0
        fi
        sleep 10
    done

    echo "Timed out waiting for SSH on ${PUBLIC_IP}" >&2
    exit 1
}

echo "Giving cloud-init and SSH a few extra seconds to settle..."
sleep 15
wait_for_ssh

echo "Stage 1: install OS packages"
ssh_box bash -s <<EOF
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" --allow-downgrades --allow-remove-essential --allow-change-held-packages \
    build-essential \
    clang \
    cmake \
    curl \
    ffmpeg \
    fish \
    git \
    gnupg \
    libglib2.0-dev \
    libgtk-3-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    linux-headers-$(uname -r) \
    lsb-release \
    pciutils \
    pkg-config \
    wget

sudo mkdir -p /root/.aws
sudo tee -a /root/.aws/credentials >/dev/null <<AWSCRED
    [default]
    aws_access_key_id = $AWS_ACCESS_KEY_ID
    aws_secret_access_key = $AWS_SECRET_ACCESS_KEY
AWSCRED

echo
echo "First-stage bootstrap complete."
sudo reboot
EOF

echo "Waiting for the box to come back after reboot..."
${AWS_BASE[@]} ec2 wait instance-status-ok --instance-ids "${INSTANCE_ID}"
wait_for_ssh

echo "Stage 2: verify GPU/toolchain, install Rust, and prepare the repo."
ssh_box bash -s <<EOF
echo "# ----------------------------------- nvcc ----------------------------------- #"
nvcc --version

echo "# -------------------------------- nvidia-smi -------------------------------- #"
nvidia-smi

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

if ! command -v ast-grep >/dev/null 2>&1; then
    cargo install ast-grep --locked
fi

if ! command -v uv >/dev/null 2>&1; then
    curl -Ls https://astral.sh/uv/install.sh | sh
fi

if ! command -v node >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    sudo mkdir -p /etc/apt/keyrings
    curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
        | sudo gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg
    echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_26.x nodistro main" \
        | sudo tee /etc/apt/sources.list.d/nodesource.list
    curl -sL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor | sudo tee /usr/share/keyrings/yarnkey.gpg >/dev/null
    echo "deb [signed-by=/usr/share/keyrings/yarnkey.gpg] https://dl.yarnpkg.com/debian stable main" | sudo tee /etc/apt/sources.list.d/yarn.list
    sudo apt-get update && sudo apt-get install -y nodejs yarn
fi

if ! command -v claude >/dev/null 2>&1; then
    curl -fsSL https://claude.ai/install.sh | bash
fi

source "\$HOME/.cargo/env"

if [[ ! -d "${AUOHP_REPO_DIR}/.git" ]]; then
    git clone "${AUOHP_REPO_URL}" "${AUOHP_REPO_DIR}"
else
    git -C "${AUOHP_REPO_DIR}" pull --ff-only
fi

cd "${AUOHP_REPO_DIR}/packages/auohp-api"

echo
echo "Ready to build. The command that should now work is:"
echo "  cargo build --release --features cuda"
EOF

cat <<EOF

Instance is up and prepared.

SSH:
    ssh -i ${AWS_SSH_KEY_PATH} ${AWS_SSH_USER}@${PUBLIC_IP}

Repo:
    ${AUOHP_REPO_DIR}

Build:
    cd ${AUOHP_REPO_DIR}/packages/auohp-api
    cargo build --release --features cuda

Stop when finixshed to avoid surprise cost:
    ${AWS_BASE[*]} ec2 stop-instances --instance-ids ${INSTANCE_ID}

Terminate when you are done for good:
    ${AWS_BASE[*]} ec2 terminate-instances --instance-ids ${INSTANCE_ID}
EOF

# Ephemeral storage
# lsblk -o NAME,SIZE,MODEL,SERIAL
# sudo nvme id-ctrl /dev/nvme1n1 | grep -i "Amazon EC2 NVMe Instance Storage"
# sudo mkfs.ext4 -E lazy_itable_init=0,lazy_journal_init=0 /dev/nvme1n1
# sudo mkdir -p /scratch
# sudo mount /dev/nvme1n1 /scratch
# sudo chown ubuntu:ubuntu /scratch
# mkdir /scratch/models /scratch/in /scratch/out /scratch/logs

# Legacy Python
# sudo add-apt-repository ppa:deadsnakes/ppa
# sudo apt-get Updater1
# sudo apt-get install -y python3.10 python3.10-venv python3.10-dev

# uv
# curl -Ls https://astral.sh/uv/install.sh | sh

# Rust
# curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile complete -y

# node.js + yarn
# sudo mkdir -p /etc/apt/keyrings
# curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
#     | sudo gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg
# echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_26.x nodistro main" \
#     | sudo tee /etc/apt/sources.list.d/nodesource.list
# curl -sL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor | sudo tee /usr/share/keyrings/yarnkey.gpg >/dev/null
# echo "deb [signed-by=/usr/share/keyrings/yarnkey.gpg] https://dl.yarnpkg.com/debian stable main" | sudo tee /etc/apt/sources.list.d/yarn.list
# sudo apt-get update && sudo apt-get install -y nodejs yarn

# S3 Files
# curl https://amazon-efs-utils.aws.com/efs-utils-installer.sh | sudo sh -s -- --install
# sudo mkdir -p /mnt/s3/fs1
# sudo mount -t s3files fs-<mount id> /mnt/s3/fs1
# echo "fs-<mount id>  /mnt/s3/fs1  s3files  _netdev,nofail  0  0" | sudo tee -a /etc/fstab
# sudo systemctl daemon-reload
# sudo umount /mnt/s3/fs1 && sudo mount /mnt/s3/fs1
