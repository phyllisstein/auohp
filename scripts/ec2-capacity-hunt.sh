#!/usr/bin/env bash
#
# Try to launch exactly one On-Demand GPU instance, sweeping regions and
# availability zones until one has capacity. No user data: bootstrap by hand.
#
#   scripts/ec2-capacity-hunt.sh                # one sweep, then exit
#   scripts/ec2-capacity-hunt.sh --watch 120    # sweep every 120s until one launches
#
# Exit codes: 0 = an instance is live (launched now or found already running),
# 1 = no capacity this sweep, 2 = configuration error.
#
# Safe to run from several places at once (cloud session, local CLI): before
# every launch attempt it looks for an existing instance of this type with this
# key pair in any listed region and stops if it finds one. Two runners landing
# in the same second can still both launch; the window is small.

set -euo pipefail

INSTANCE_TYPE="${AWS_INSTANCE_TYPE:-g7.2xlarge}"
KEY_NAME="${AWS_KEY_NAME:-AUOHP}"
SECURITY_GROUP_NAME="${AWS_SECURITY_GROUP_NAME:-default}"
read -ra REGIONS <<<"${AWS_HUNT_REGIONS:-us-east-1 us-west-2 us-east-2}"
AMI_NAME_PATTERN="${AMI_NAME_PATTERN:-Deep Learning Base AMI with Single CUDA (Ubuntu 24.04)*}"
TAG_VALUE=auohp-capacity-hunt

log() { printf '%s  %s\n' "$(date -u +%H:%M:%SZ)" "$*" >&2; }

# Print "region instance-id state" for any live instance we'd count as success.
# Fails if any region can't be checked, so callers never mistake an API error
# for "nothing running" and launch a duplicate.
find_existing() {
    local region found
    for region in "${REGIONS[@]}"; do
        found="$(aws ec2 describe-instances --region "$region" \
            --filters "Name=instance-type,Values=$INSTANCE_TYPE" \
                      "Name=key-name,Values=$KEY_NAME" \
                      "Name=instance-state-name,Values=pending,running" \
            --query 'Reservations[].Instances[].[InstanceId,State.Name]' \
            --output text)" || return 1
        [[ -z "$found" ]] || printf '%s %s\n' "$region" "$found"
    done
}

latest_ami() {
    aws ec2 describe-images --region "$1" --owners amazon \
        --filters "Name=name,Values=$AMI_NAME_PATTERN" \
                  "Name=architecture,Values=x86_64" \
                  "Name=state,Values=available" \
        --query 'sort_by(Images,&CreationDate)[-1].ImageId' --output text
}

default_vpc() {
    aws ec2 describe-vpcs --region "$1" --filters Name=is-default,Values=true \
        --query 'Vpcs[0].VpcId' --output text
}

security_group() {
    aws ec2 describe-security-groups --region "$1" \
        --filters "Name=group-name,Values=$SECURITY_GROUP_NAME" "Name=vpc-id,Values=$2" \
        --query 'SecurityGroups[0].GroupId' --output text
}

# Availability zones in the region that offer the instance type at all.
offering_zones() {
    aws ec2 describe-instance-type-offerings --region "$1" \
        --location-type availability-zone \
        --filters "Name=instance-type,Values=$INSTANCE_TYPE" \
        --query 'InstanceTypeOfferings[].Location' --output text
}

default_subnet() {
    aws ec2 describe-subnets --region "$1" \
        --filters "Name=vpc-id,Values=$2" "Name=availability-zone,Values=$3" \
                  Name=default-for-az,Values=true \
        --query 'Subnets[0].SubnetId' --output text
}

# Returns 0 on launch, 1 when every zone refused, 2 on a configuration error.
try_region() {
    local region="$1" ami vpc sg zones zone subnet out existing

    ami="$(latest_ami "$region")"
    vpc="$(default_vpc "$region")"
    [[ "$ami" == ami-* ]] || { log "$region: no AMI matching '$AMI_NAME_PATTERN'"; return 2; }
    [[ "$vpc" == vpc-* ]] || { log "$region: no default VPC"; return 2; }
    sg="$(security_group "$region" "$vpc")"
    [[ "$sg" == sg-* ]] || { log "$region: no '$SECURITY_GROUP_NAME' security group in $vpc"; return 2; }
    zones="$(offering_zones "$region")"
    [[ -n "$zones" ]] || { log "$region: $INSTANCE_TYPE not offered"; return 1; }

    for zone in $zones; do
        subnet="$(default_subnet "$region" "$vpc" "$zone")"
        [[ "$subnet" == subnet-* ]] || { log "$zone: no default subnet, skipping"; continue; }

        existing="$(find_existing)" || { log "couldn't check for existing instances"; return 1; }
        if [[ -n "$existing" ]]; then
            log "another runner launched first: $existing"
            return 0
        fi

        if out="$(aws ec2 run-instances --region "$region" \
            --image-id "$ami" \
            --instance-type "$INSTANCE_TYPE" \
            --key-name "$KEY_NAME" \
            --security-group-ids "$sg" \
            --subnet-id "$subnet" \
            --count 1 \
            --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$TAG_VALUE}]" \
            --query 'Instances[0].InstanceId' --output text 2>&1)"; then
            log "LAUNCHED $out in $zone ($ami)"
            echo "$region $out"
            return 0
        fi

        case "$out" in
            *InsufficientInstanceCapacity*) log "$zone: no capacity" ;;
            *VcpuLimitExceeded*)            log "$region: On-Demand G vCPU quota too low"; return 1 ;;
            *Unsupported*)                  log "$zone: unsupported" ;;
            *)                              log "$zone: ${out//$'\n'/ }" ;;
        esac
    done
    return 1
}

sweep() {
    local existing region rc
    existing="$(find_existing)" || { log "couldn't check for existing instances"; return 1; }
    if [[ -n "$existing" ]]; then
        log "already live: $existing"
        return 0
    fi
    for region in "${REGIONS[@]}"; do
        rc=0
        try_region "$region" || rc=$?
        [[ $rc -eq 0 ]] && return 0
    done
    return 1
}

main() {
    local interval=""
    if [[ "${1:-}" == "--watch" ]]; then
        interval="${2:-120}"
    fi

    aws sts get-caller-identity >/dev/null || { log "AWS credentials rejected"; exit 2; }

    if [[ -z "$interval" ]]; then
        sweep
        exit
    fi
    until sweep; do
        sleep "$interval"
    done
}

main "$@"
