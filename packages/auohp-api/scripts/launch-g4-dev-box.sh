#!/usr/bin/env bash

set -euo pipefail

# Quick-and-dirty AWS CLI bootstrap for an AUOHP GPU dev box.
#
# What this script does:
# - Resolves the current Ubuntu 22.04 LTS AMI via AWS SSM.
# - Launches a g4dn.xlarge instance.
# - Waits for the instance to come up.
# - SSHes in to install build tools, CUDA 12.6, and the NVIDIA server driver.
# - Reboots the machine once, because the driver/kernel integration is not
#   reliably usable until after a reboot.
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

# Optional inputs.
: "${AWS_INSTANCE_NAME:=auohp-g4-dev}"
: "${AWS_INSTANCE_TYPE:=g4dn.xlarge}"
: "${AWS_DISK_SIZE_GB:=200}"
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

Optional but usually useful:
    AWS_PROFILE=default
    AWS_REGION=us-east-2
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

echo "Resolving the current Ubuntu 22.04 AMI from AWS SSM..."
AMI_ID="$(${AWS_BASE[@]} ssm get-parameter \
    --name /aws/service/canonical/ubuntu/server/22.04/stable/current/amd64/hvm/ebs-gp3/ami-id \
    --query 'Parameter.Value' \
    --output text)"

echo "Using AMI: ${AMI_ID}"

RUN_ARGS=(
    ec2 run-instances
    --image-id "${AMI_ID}"
    --instance-type "${AWS_INSTANCE_TYPE}"
    --key-name "${AWS_KEY_NAME}"
    --security-group-ids "${AWS_SECURITY_GROUP_ID}"
    --subnet-id "${AWS_SUBNET_ID}"
    --block-device-mappings "[{\"DeviceName\":\"/dev/sda1\",\"Ebs\":{\"VolumeSize\":${AWS_DISK_SIZE_GB},\"VolumeType\":\"gp3\"}}]"
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=${AWS_INSTANCE_NAME}}]"
    --query 'Instances[0].InstanceId'
    --output text
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

echo "Stage 1: install OS packages, CUDA 12.6 toolkit, and NVIDIA server driver."
ssh_box bash -s <<EOF
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    clang \
    cmake \
    curl \
    git \
    gnupg \
    libssl-dev
    linux-headers-aws \
    lsb-release \
    pciutils \
    pkg-config \
    wget

# Install the NVIDIA CUDA apt repo. We pin to 12.6 because CUDA 13 rejected the
# compute_50 target emitted by whisper.cpp's default architecture matrix.
wget -q https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
rm -f cuda-keyring_1.1-1_all.deb

sudo apt-get update
sudo apt-get install -y cuda-toolkit-12-6 nvidia-driver-570-server

# Make the chosen toolkit easy to find in fresh login shells.
grep -q cuda-12.6 ~/.bashrc || cat >> ~/.bashrc <<EOF
export PATH=/usr/local/cuda-12.6/bin:\$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-12.6/lib64:\$LD_LIBRARY_PATH
EOF

echo
echo "First-stage bootstrap complete. Rebooting so the NVIDIA kernel driver loads cleanly."
sudo reboot
EOF

echo "Waiting for the box to come back after reboot..."
${AWS_BASE[@]} ec2 wait instance-status-ok --instance-ids "${INSTANCE_ID}"
wait_for_ssh

echo "Stage 2: verify GPU/toolchain, install Rust, and prepare the repo."
ssh_box bash -s <<EOF
set -euo pipefail

export PATH=/usr/local/cuda-12.6/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-12.6/lib64:$LD_LIBRARY_PATH

echo "== nvcc =="
nvcc --version

echo "== nvidia-smi =="
nvidia-smi

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain nightly --profile complete -y
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

Stop when finished to avoid surprise cost:
    ${AWS_BASE[*]} ec2 stop-instances --instance-ids ${INSTANCE_ID}

Terminate when you are done for good:
    ${AWS_BASE[*]} ec2 terminate-instances --instance-ids ${INSTANCE_ID}
EOF

# Ephemeral storage
# lsblk -o NAME,SIZE,MODEL,SERIAL
# sudo nvme id-ctrl /dev/nvme1n1 | grep -i "Amazon EC2 NVMe Instance Storage"
# sudo mkfs.ext4 -E lazy_itable_init=0,lazy_journal_init=0 /dev/nvme1n1
# mkdir -p /mnt/scratch
# sudo mount /dev/nvme1n1 /mnt/scratch
# sudo chown ubuntu:ubuntu /mnt/scratch
