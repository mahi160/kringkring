# 03: Local CA + HTTPS

**What to build:**

On first run the binary creates a root CA (10-year validity) and a server cert signed by it (max 825 days, SAN = configured static IP + `kringkring.local`, EKU serverAuth), stored in the data dir with mode 600. The server cert is re-issued automatically when within 30 days of expiry or when the configured IP/SANs change — the CA is never regenerated.

`:443` serves the app over TLS (rustls). `:80` serves only: a CA download page (`.crt` for Android, `.mobileconfig` for iOS) with a QR code to itself, and redirects everything else to HTTPS.

Use the exact install steps recorded by ticket 01. See SPEC.md §8.

**Blocked by:** 01 (iOS/Android push spike), 02 (Project skeleton)

**Status:** superseded: the office gateway terminates HTTPS for `kringkring.questionpro.lan` with the office CA, so the app's own CA was removed.

- [x] Fresh data dir → CA + cert created; restart reuses them (test)
- [x] Cert SANs contain the IP and `kringkring.local`; validity ≤ 825 days (test with parsed cert)
- [x] Near-expiry or changed-IP cert is re-issued with the same CA (test)
- [x] `:80/` shows the CA page with both downloads; any other `:80` path → 301 to https
- [ ] `.mobileconfig` installs on iOS; `.crt` installs on Android (manual check)
