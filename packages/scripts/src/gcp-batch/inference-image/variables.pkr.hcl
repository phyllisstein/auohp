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
  default = "n1-standard-1"
}

variable "source_image" {
  type    = string
  default = "c2-deeplearning-pytorch-2-3-cu121-v20240730-debian-11-py310"
}

variable "debug" {
  type    = bool
  default = false
}
