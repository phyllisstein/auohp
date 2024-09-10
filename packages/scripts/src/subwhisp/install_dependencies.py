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

        # Install common dependencies using conda
        subprocess.check_call(["conda", "env", "update", "--name", "base", "--file", "environment.yml", "--yes"])

        if is_pytorch_installed():
            print("PyTorch is already installed. Installing additional PyTorch-related modules.")
            subprocess.check_call(["conda", "install", "-y", "torchvision", "torchaudio"])
        else:
            try:
                import torch
                if torch.cuda.is_available():
                    print("CUDA is available. Installing CUDA-enabled PyTorch and related modules.")
                    subprocess.check_call(["conda", "install", "-y", "pytorch", "torchvision", "torchaudio", "cudatoolkit=12.1", "-c", "pytorch", "-c", "nvidia"])
                else:
                    print("CUDA is not available. Installing CPU-only PyTorch and related modules.")
                    subprocess.check_call(["conda", "install", "-y", "pytorch", "torchvision", "torchaudio", "-c", "pytorch"])
            except ImportError:
                print("PyTorch is not installed. Installing CPU-only PyTorch and related modules.")
                subprocess.check_call(["conda", "install", "-y", "pytorch", "torchvision", "torchaudio", "-c", "pytorch"])
    except Exception as e:
        print(f"Error installing dependencies: {e}")


if __name__ == "__main__":
    install_dependencies()
