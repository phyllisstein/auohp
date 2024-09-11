source "googlecompute" "auohp_inference" {
  project_id       = var.project_id
  credentials_file = var.credentials_file

  source_image      = var.source_image
  zone              = var.zone
  machine_type      = var.machine_type
  image_name        = var.image_name
  image_description = "Debian/CUDA/PyTorch inference image (${timestamp()})"
  disk_size         = 100

  ssh_username            = "packer"
  temporary_key_pair_type = "ed25519"
  use_os_login            = true

  # accelerator_type  = "projects/${var.project_id}/zones/${var.zone}/acceleratorTypes/nvidia-tesla-t4"
  # accelerator_count = 1

  preemptible         = true
  on_host_maintenance = "TERMINATE"

  skip_create_image = var.debug

  metadata = {
    "block-project-ssh-keys" = "TRUE"
    "ssh-keys"               = "auohp:${var.ssh_public_key}"
  }
}

build {
  sources = ["source.googlecompute.auohp_inference"]

  provisioner "shell" {
    environment_vars = [
      "DEBIAN_FRONTEND=noninteractive"
    ]
    inline = [
      "sudo apt-get update",
      "sudo apt-get upgrade -y",
      "sudo apt-get install -y fish git ffmpeg",
      "sudo /opt/deeplearning/install-driver.sh"
    ]
  }

  provisioner "shell" {
    scripts = [
      "./scripts/00-bootstrap.fish",
      "./scripts/10-create-user.fish",
    ]
  }

  provisioner "shell" {
    environment_vars = [
      "AUOHP_GIT_REPO_URL=${var.auohp_git_repo_url}",
      "GIT_SSH_KEY_BASE64=${var.git_ssh_key}",
      "AUOHP_PUBLIC_KEY=${var.ssh_public_key}"
    ]
    execute_command = "sudo -u auohp -S fish -c '{{ .Vars }} {{ .Path }}'"
    scripts = [
      "./scripts/20-clone-repo.fish",
      "./scripts/30-configure-python.fish",
      "./scripts/40-cleanup.fish"
    ]
  }
}
