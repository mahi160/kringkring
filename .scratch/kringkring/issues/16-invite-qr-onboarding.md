# 16: Invite QR onboarding

**What to build:**

Admin user page shows a personal **invite QR** (single-use token, 24h expiry, regenerating invalidates the old one). The QR points to `http://<ip>/invite/<token>` (plain HTTP because the CA isn't trusted yet). Guided "Step N of 4" flow:
1. install CA (per-OS steps from ticket 03),
2. open the HTTPS URL and Add to Home Screen (guide from 15),
3. username prefilled; set PIN (if admin left it unset) or enter it,
4. enable notifications.

The token is carried across the HTTP→HTTPS hop and consumed only on successful login.

See SPEC.md §3 Onboarding.

**Blocked by:** 03 (Local CA + HTTPS), 07 (Admin: users), 15 (Install guide + notification nag)

**Status:** ready-for-agent

- [ ] Expired or used token shows a clear error (tests)
- [ ] Regenerating invalidates the previous token (test)
- [ ] Full flow on a fresh iPhone and Android ends logged in with push enabled (manual)
- [ ] Token is not logged
