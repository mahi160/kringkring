#!/usr/bin/env bash
# Runs infra/install.sh on the Proxmox host. Builds come from GitHub (CI on push to main);
# the container pulls them itself, so this is only needed once per container.
#   bash deploy.sh               create a new container
#   bash deploy.sh upgrade CTID  convert an existing container, keeping its data
set -euo pipefail

fail() { printf '%s\n' "$*" >&2; exit 1; }
[[ -t 0 ]] || fail 'Run bash deploy.sh in an interactive terminal.'
case $# in
    0) args=(create) ;;
    2) [[ $1 == upgrade && $2 =~ ^[1-9][0-9]{2,8}$ ]] || fail 'Usage: bash deploy.sh [upgrade CTID]'
       args=(upgrade "$2") ;;
    *) fail 'Usage: bash deploy.sh [upgrade CTID]' ;;
esac
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
host=root@qpbd.local
work=$(mktemp -d /tmp/kringkring-ssh.XXXXXX)
socket="$work/ssh"
stage=''
cleanup() {
    if [[ -n $stage ]]; then
        ssh -S "$socket" -o BatchMode=yes "$host" \
            "rm -f -- '$stage/install.sh' '$stage/kringkring-update'; rmdir -- '$stage'" \
            </dev/null >/dev/null 2>&1 || true
    fi
    ssh -S "$socket" -O exit "$host" </dev/null >/dev/null 2>&1 || true
    rmdir -- "$work" 2>/dev/null || true
}
trap cleanup EXIT
opts=(-o StrictHostKeyChecking=yes -o "ControlPath=$socket")
ssh "${opts[@]}" -M -o ControlPersist=no -fnNT "$host"
tmp=$(ssh "${opts[@]}" -T -o BatchMode=yes "$host" 'mktemp -d /tmp/kringkring-deploy.XXXXXX' </dev/null)
[[ $tmp =~ ^/tmp/kringkring-deploy\.[a-zA-Z0-9]{6}$ ]] || fail 'Invalid remote staging directory.'
stage=$tmp
scp "${opts[@]}" -o BatchMode=yes infra/install.sh infra/kringkring-update "$host:$stage/"
ssh "${opts[@]}" -t -o BatchMode=yes "$host" "bash '$stage/install.sh' ${args[*]}"
