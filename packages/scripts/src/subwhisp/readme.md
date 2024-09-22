# `@auohp/subwhisp`
![](mascot.gif)

## Getting Started
1. Install bulk of dependencies into an Anaconda virtualenv.

    ```shell
    conda create -n auohp
    conda activate auohp
    conda env update -f environment.yml
    conda env export --no-builds -f environment.yml
    ```

2. Add pytorch and related dependencies.

    ```shell
    # with Nvidia GPU and CUDA libraries
    python install_dependencies.py --cuda
    
    # with CPU-only pytorch
    python install_dependencies.py --no-cuda
    ```
    
3. Optionally, use Poetry to set up an executable.

    ```shell
    poetry config virtualenvs.create false
    poetry install
    ```


## Running Transcriptions
