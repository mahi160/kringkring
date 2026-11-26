#!/usr/bin/env bash
# Run as root on the Proxmox host, next to kringkring-update:
#   install.sh create          new container `kringkring` with a separate data volume
#   install.sh upgrade CTID    turn an existing container into `kringkring`, keeping its data
# Both install the OpenRC service and a 5-minute updater that pulls the latest main build
# from GitHub. The office gateway serves it as https://kringkring.questionpro.lan.
set -euo pipefail

name=kringkring
data=/var/lib/kringkring
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

fail() { printf '%s\n' "$*" >&2; exit 1; }
# Prompts go to stderr so callers can capture the answer.
ask() {
    local reply
    read -r -p "$1 [$2]: " reply
    printf '%s' "${reply:-$2}"
}
ask_contact() {
    local contact
    contact=$(ask 'Admin email for push services' "$1")
    [[ $contact =~ ^[^[:space:]\"@]+@[^[:space:]\"@]+\.[^[:space:]\"@]+$ ]] || fail 'Invalid email address.'
    printf '%s' "$contact"
}
# Another container already answers as kringkring.lxc.
name_taken() {
    pct list | awk -v id="$1" -v name="$name" 'NR > 1 && $1 != id && $NF == name {found = 1} END {exit !found}'
}

if [[ $EUID != 0 ]] || ! command -v pct >/dev/null || ! command -v pveam >/dev/null; then
    fail 'Run this script as root on the Proxmox host.'
fi
[[ -f $here/kringkring-update ]] || fail "Missing $here/kringkring-update."
case $(uname -m) in
    x86_64) arch=amd64 ;;
    aarch64) arch=arm64 ;;
    *) fail 'Unsupported Proxmox host architecture.' ;;
esac

# Installs the service, its settings, and the updater, then installs the latest build.
setup() {
    local ctid=$1 contact=$2 files
    printf 'Waiting for the network in CT %s...\n' "$ctid"
    for _ in $(seq 30); do
        pct exec "$ctid" -- nslookup github.com >/dev/null 2>&1 && break
        sleep 2
    done
    pct exec "$ctid" -- apk add -q ca-certificates ssl_client
    files=$(mktemp -d)
    cat >"$files/conf" <<EOF
# Written by install.sh.
KRINGKRING_PUSH_CONTACT="mailto:$contact"
EOF
    cat >"$files/init" <<'EOF'
#!/sbin/openrc-run
description="kringkring request server"
supervisor=supervise-daemon
command=/usr/local/bin/kringkring
command_args="serve --data-dir /var/lib/kringkring --host 0.0.0.0 --port 80 --push-contact ${KRINGKRING_PUSH_CONTACT}"
command_user="kringkring:kringkring"
capabilities="^cap_net_bind_service"
output_log=/var/log/kringkring.log
error_log=/var/log/kringkring.log
respawn_delay=2

depend() {
    use net
}

start_pre() {
    checkpath -f -m 0640 -o kringkring:kringkring /var/log/kringkring.log
}
EOF
    pct exec "$ctid" -- sh -eu -c '
        rc-service kringkring stop >/dev/null 2>&1 || true
        pkill -x kringkring || true
        grep -q "^kringkring:" /etc/group || addgroup -S kringkring
        id kringkring >/dev/null 2>&1 || adduser -S -D -H -h /var/lib/kringkring -s /sbin/nologin -G kringkring kringkring
        mkdir -p /usr/local/bin /usr/local/sbin /var/lib/kringkring'
    pct push "$ctid" "$files/conf" /etc/conf.d/kringkring --perms 0644
    pct push "$ctid" "$files/init" /etc/init.d/kringkring --perms 0755
    pct push "$ctid" "$here/kringkring-update" /usr/local/sbin/kringkring-update --perms 0755
    rm -rf -- "$files"
    pct exec "$ctid" -- sh -eu -c "
        chown -R kringkring:kringkring $data
        chmod 0700 $data
        rc-update add kringkring default >/dev/null
        rc-update add crond default >/dev/null
        line='*/5 * * * * /usr/local/sbin/kringkring-update >>/var/log/kringkring-update.log 2>&1'
        grep -qF kringkring-update /etc/crontabs/root || echo \"\$line\" >>/etc/crontabs/root
        rc-service crond restart >/dev/null
        rm -f /usr/local/lib/kringkring/deployed /usr/local/lib/kringkring/failed"
    printf 'Installing the latest build from GitHub...\n'
    pct exec "$ctid" -- /usr/local/sbin/kringkring-update ||
        fail "CT $ctid could not install the latest build; see /var/log/kringkring.log in the CT."
    pct exec "$ctid" -- test -s /usr/local/lib/kringkring/deployed ||
        fail 'No build is published yet. Push to main and wait for CI, then run this again.'
}

create() {
    local ctid storage templates bridge contact username pin again template filename
    name_taken '' && fail "A container named $name already exists. Use: install.sh upgrade CTID"
    ctid=$(ask 'Container ID' "$(pvesh get /cluster/nextid)")
    [[ $ctid =~ ^[1-9][0-9]{2,8}$ ]] || fail 'Invalid container ID.'
    pvesh get /cluster/nextid --vmid "$ctid" >/dev/null 2>&1 || fail "CT $ctid already exists."
    pvesm status
    storage=$(ask 'Disk storage' local-lvm)
    templates=$(ask 'Template storage' local)
    bridge=$(ask 'Bridge' vmbr0)
    for value in "$storage" "$templates" "$bridge"; do
        [[ $value =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] || fail "Invalid name: $value"
    done
    contact=$(ask_contact '')
    username=$(ask 'First admin username' admin)
    [[ -n $username ]] || fail 'Enter a username.'
    read -r -s -p 'First admin four-digit PIN: ' pin; printf '\n'
    read -r -s -p 'Repeat the PIN: ' again; printf '\n'
    [[ $pin =~ ^[0-9]{4}$ && $pin == "$again" ]] || fail 'The PINs must match and be exactly four digits.'

    printf '\nCreate Alpine CT %s (%s, DHCP on %s) with a 1 GiB data volume on %s?\n' \
        "$ctid" "$name" "$bridge" "$storage"
    [[ $(ask 'Type yes to create it' no) == yes ]] || fail 'Cancelled before creating a container.'

    template=$(pveam list "$templates" | awk -v arch="$arch" '$1 ~ (":vztmpl/alpine-.*_" arch "\\.tar\\.(xz|gz|zst)$") {print $1}' | sort -V | tail -n 1)
    if [[ -z $template ]]; then
        pveam update
        filename=$(pveam available --section system | awk -v arch="$arch" '$2 ~ ("^alpine-.*_" arch "\\.tar\\.(xz|gz|zst)$") {print $2}' | sort -V | tail -n 1)
        [[ -n $filename ]] || fail "No Alpine template found for $arch."
        pveam download "$templates" "$filename"
        template="$templates:vztmpl/$filename"
    fi
    # The data volume is separate so the container can be rebuilt around it.
    pct create "$ctid" "$template" --hostname "$name" --ostype alpine --arch "$arch" \
        --unprivileged 1 --cores 1 --memory 64 --swap 64 --onboot 1 \
        --rootfs "$storage:1" --mp0 "$storage:1,mp=$data,backup=1" \
        --net0 "name=eth0,bridge=$bridge,ip=dhcp" \
        --description 'ক্রিং ক্রিং - https://kringkring.questionpro.lan'
    printf 'Created CT %s; it is left for inspection if a later step fails.\n' "$ctid"
    pct start "$ctid"
    setup "$ctid" "$contact"
    # The guest shell reads the PIN from stdin so it never appears in a process list.
    # shellcheck disable=SC2016
    printf '%s\n%s\n' "$username" "$pin" | pct exec "$ctid" -- sh -eu -c '
        IFS= read -r username
        IFS= read -r pin
        su -s /bin/sh kringkring -c "umask 077; /usr/local/bin/kringkring init --data-dir /var/lib/kringkring --username \"\$0\" --pin \"\$1\"" "$username" "$pin"'
    unset pin again
    done_message "$ctid"
}

upgrade() {
    [[ $# -eq 1 && $1 =~ ^[1-9][0-9]{2,8}$ ]] || fail 'Usage: install.sh upgrade CTID'
    local ctid=$1 bridge existing contact
    pct status "$ctid" >/dev/null || fail "CT $ctid does not exist."
    name_taken "$ctid" && fail "Another container is already named $name."
    if ! pct config "$ctid" | grep -q '^net0:'; then
        bridge=$(ask 'Bridge' vmbr0)
        [[ $bridge =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] || fail 'Invalid bridge name.'
        pct set "$ctid" --net0 "name=eth0,bridge=$bridge,ip=dhcp"
    fi
    pct set "$ctid" --hostname "$name" --onboot 1
    # Restart so the new hostname and network apply inside the container.
    if [[ $(pct status "$ctid") == 'status: running' ]]; then pct reboot "$ctid"; else pct start "$ctid"; fi
    pct exec "$ctid" -- test -f "$data/kringkring.sqlite3" ||
        fail "CT $ctid has no database in $data; use create for a new install."
    # shellcheck disable=SC2016
    existing=$(pct exec "$ctid" -- sh -c '. /etc/conf.d/kringkring 2>/dev/null; printf %s "${KRINGKRING_PUSH_CONTACT#mailto:}"')
    contact=$(ask_contact "$existing")
    setup "$ctid" "$contact"
    done_message "$ctid"
}

done_message() {
    printf '\nক্রিং ক্রিং is running in CT %s and updates itself within 5 minutes of each push to main.\n' "$1"
    printf 'Within a minute it is at https://%s.questionpro.lan/\n' "$name"
    printf 'Phones must trust the office certificate first: https://hub.questionpro.lan/announcements/trust-the-office-certificate-for-questionprolan\n'
}

case ${1:-} in
    create) [[ $# -eq 1 ]] || fail 'Usage: install.sh create'; create ;;
    upgrade) shift; upgrade "$@" ;;
    *) fail 'Usage: install.sh create | install.sh upgrade CTID' ;;
esac
