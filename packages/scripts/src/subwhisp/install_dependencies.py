import subprocess
import os
import argparse

def install_dependencies(cuda_enabled):
    try:
        # Check if Conda is available
        subprocess.check_call(["conda", "--version"])

        # Update conda
        subprocess.check_call(["conda", "update", "-y", "-n", "base", "-c", "conda-forge", "conda"])

        # Install other dependencies
        subprocess.check_call(["conda", "env", "update", "--name", "base", "--file", "environment.yml"])

        if cuda_enabled:
            print("CUDA is enabled. Installing CUDA-enabled PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch=2.3", "torchvision", "torchaudio", "cudatoolkit=12.1", "-c", "pytorch", "-c", "nvidia"])
        else:
            print("CUDA is not enabled. Installing CPU-only PyTorch and related modules.")
            subprocess.check_call(["conda", "install", "--update-deps", "-y", "pytorch=2.3", "torchvision", "torchaudio", "-c", "pytorch"])

    except Exception as e:
        print(f"Error installing dependencies: {e}")

def main():
    parser = argparse.ArgumentParser(description="Install dependencies for the project.")
    parser.add_argument("--cuda", action="store_true", help="Install CUDA-enabled dependencies.")
    parser.add_argument("--no-cuda", action="store_true", help="Install CPU-only dependencies.")
    args = parser.parse_args()

    if args.cuda and args.no_cuda:
        print("Error: Both --cuda and --no-cuda options cannot be specified at the same time.")
        return

    if not args.cuda and not args.no_cuda:
        print("No CUDA option specified. Installing CPU-only dependencies")
        install_dependencies(cuda_enabled=False)
        return

    install_dependencies(cuda_enabled=args.cuda)

if __name__ == "__main__":
    main()
