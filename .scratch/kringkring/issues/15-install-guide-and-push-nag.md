# 15: Install guide + notification nag

**What to build:**

If the app is opened in a normal browser tab (not standalone display mode), show a full-screen per-OS "Add to Home Screen" guide with illustrations. Inside the installed PWA, if notification permission is not granted or there is no subscription for this device, show a persistent banner with an **Enable notifications** button that never goes away until enabled. Both bilingual.

See SPEC.md §5 Push nag.

**Blocked by:** 13 (Web Push)

**Status:** ready-for-agent

- [ ] iOS Safari tab shows the iOS guide; Android Chrome tab shows the Android guide (manual)
- [ ] Installed app without permission shows the banner; granting removes it
- [ ] Revoking permission in OS settings brings the banner back on next open
