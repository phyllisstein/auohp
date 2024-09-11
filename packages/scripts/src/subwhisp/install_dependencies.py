import subprocess

def is_pytorch_installed():
    try:
        import torch
        print(f"PyTorch version {torch.__version__} is already installed.")
        return True
    except ImportError as e:
        print(f"PyTorch is not installed: {e}")
        return False


def install_dependencies():
    try:
        # Check if Conda is available
        subprocess.check_call(["conda", "--version"])

        # Update conda
        # subprocess.check_call(["conda", "update", "-y", "-n", "base", "-c", "conda-forge", "conda"])

        # Install other dependencies
        subprocess.check_call(["conda", "env", "update", "--name", "base", "--file", "environment.yml"])
        subprocess.check_call(["poetry", "config", "virtualenvs.create", "false"])
        subprocess.check_call(["poetry", "install"])

        cuda_check = subprocess.run(["nvcc", "--version"], capture_output=True, text=True, shell=True)
        if cuda_check.returncode == 0:
            print("CUDA is available. Installing CUDA-enabled PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch", "torchvision", "torchaudio", "cudatoolkit=12.1", "-c", "pytorch", "-c", "nvidia"])
        else:
            print("CUDA is not available. Installing CPU-only PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch", "torchvision", "torchaudio", "-c", "pytorch"])

    except Exception as e:
        print(f"Error installing dependencies: {e}")


if __name__ == "__main__":
    install_dependencies()
