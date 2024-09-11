```shell
export PROJECT_ID=
export COMPUTE_SERVICE_ACCOUNT_EMAIL=

gcloud iam service-accounts create packer-builder --display-name "Packer Builder"
gcloud projects add-iam-policy-binding $PROJECT_ID --member="serviceAccount:packer-builder@${PROJECT_ID}.iam.gserviceaccount.com" --role="roles/compute.instanceAdmin.v1"
gcloud projects add-iam-policy-binding $PROJECT_ID --member="serviceAccount:packer-builder@${PROJECT_ID}.iam.gserviceaccount.com" --role="roles/compute.admin"
gcloud projects add-iam-policy-binding $COMPUTE_SERVICE_ACCOUNT_EMAIL --member="serviceAccount:packer-builder@${PROJECT_ID}.iam.gserviceaccount.com" --role="roles/iam.serviceAccountUser"
gcloud compute os-login ssh-keys add --key-file ../../secrets/id_ed25519.pub
```

```hcl
# inference-image/variables.auto.pkrvars.hcl
project_id         = ""
zone               = "us-east1-a"
```

```shell
packer plugins install github.com/hashicorp/googlecompute
packer build -force inference-image
```

```shell

```
