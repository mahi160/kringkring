# 22: Proxmox installer

**What to build:**

`install.sh` run on the Proxmox host:
- prompts (with defaults) for CTID, storage, bridge, static IP/CIDR + gateway; suggests a high free IP and checks it with arping; warns it should be outside DHCP;
- creates an unprivileged Alpine LXC: 1 core, 64 MB RAM, 256 MB rootfs, plus a separate mount point volume at `/var/lib/kringkring` with backup=1;
- downloads the latest release binary (verifies checksum), installs an OpenRC service, sets the IP in config;
- prompts for first admin username + PIN and runs `kringkring init`;
- prints the setup URL and the CA page URL.

`install.sh update <CTID>` replaces the binary and restarts. Re-running create on an existing CTID refuses. Destroying and recreating the container with the same data volume keeps all data.

See SPEC.md §9.

**Blocked by:** 03 (Local CA + HTTPS), 04 (Users, PIN login, sessions), 21 (Release builds)

**Status:** in progress (CT 107 upgraded; a fresh `create` and an end-to-end auto-update of a changed binary are untested)

Revised: the container is named `kringkring` on DHCP; the office gateway serves it as `https://kringkring.questionpro.lan`. Each push to `main` publishes the `kringkring-build` prerelease, and the container's cron job (`infra/kringkring-update`) installs it within 5 minutes, rolling back if `/health` fails.

- [ ] Fresh run on a Proxmox host yields a working app at https://kringkring.questionpro.lan (manual)
- [x] `upgrade 107` converts CT 107 without data loss (DB and backups kept; serves https://kringkring.questionpro.lan)
- [ ] A push to main reaches the container within 5 minutes
- [ ] Recreate container + reattach volume keeps users. Not automated; see README.
- [x] Scripts pass `shellcheck`
- [x] Idle RAM of the service fits comfortably in 64 MB: 7.1 MB RSS in CT 107
