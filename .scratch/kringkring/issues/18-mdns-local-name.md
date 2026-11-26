# 18: kringkring.local name

**What to build:**

Built-in mDNS responder announcing `kringkring.local` → the configured IP (pure Rust crate, no avahi). Cert SAN already includes the name (ticket 03). Can be disabled by config.

See SPEC.md §8.

**Blocked by:** 03 (Local CA + HTTPS)

**Status:** superseded: the office DNS serves `kringkring.questionpro.lan`, so the mDNS responder was removed.

- [x] `ping kringkring.local` resolves from a Mac on the LAN
- [ ] iPhone opens `https://kringkring.local` without cert warning (manual)
- [x] Disabled by config → no announcements (`--mdns false` / `KRINGKRING_MDNS=false`)

Notes: `mdns-sd` 0.22 registers `_https._tcp` instance `kringkring` with host `kringkring.local.` and answers A queries for it. Only runs with `--ip`. Goodbye packets are sent only on a clean drop; after a kill, caches forget the name within the 120 s host TTL.
