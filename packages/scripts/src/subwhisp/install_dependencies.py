import subprocess
import os

def is_cuda_available():
    # Check for nvcc
    nvcc_check = subprocess.run(["nvcc", "--version"], capture_output=True, text=True, shell=True)
    if nvcc_check.returncode == 0:
        return True

    # Check for common CUDA library files
    cuda_libs = ["/usr/local/cuda/lib64/libcudart.so", "/usr/local/cuda/lib64/libcudart.so.10.1"]
    for lib in cuda_libs:
        if os.path.isfile(lib):
            return True

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

        if is_cuda_available():
            print("CUDA is available. Installing CUDA-enabled PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch=2.3", "torchvision", "torchaudio", "cudatoolkit=12.1", "-c", "pytorch", "-c", "nvidia"])
        else:
            print("CUDA is not available. Installing CPU-only PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch=2.3", "torchvision", "torchaudio", "-c", "pytorch"])

    except Exception as e:
        print(f"Error installing dependencies: {e}")

if __name__ == "__main__":
    install_dependencies()
