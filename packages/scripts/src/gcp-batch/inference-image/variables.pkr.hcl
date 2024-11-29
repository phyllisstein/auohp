variable "project_id" {
  type = string
}

variable "credentials_file" {
  type = string
}

variable "image_name" {
  type    = string
  default = "auohp-inference"
}

variable "auohp_git_repo_url" {
  type = string
}

variable "git_ssh_key" {
  type      = string
  sensitive = true
}

variable "ssh_public_key" {
  type = string
}

variable "zone" {
  type    = string
  default = "us-central1-f"
}

variable "machine_type" {
  type    = string
  default = "n1-standard-16"
}

variable "source_image" {
  type    = string
  default = "c0-deeplearning-common-gpu-v20241118-debian-11-py310"
}

variable "accelerator_type" {
  type    = string
  default = "t4"
}

variable "debug" {
  type    = bool
  default = false
}
