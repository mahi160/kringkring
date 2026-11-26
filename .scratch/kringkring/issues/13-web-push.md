# 13: Web Push

**What to build:**

Generate VAPID keys on first run (data dir, mode 600). Service worker + manifest for the real app. A subscribe endpoint stores push subscriptions per user/device; dead subscriptions (404/410) are deleted. Send pushes:
- to all **on-duty** staff: new request, employee cancel;
- to the employee: accepted, rejected, done, expired.

Push text is rendered in the **recipient's** language. Tapping a notification opens the app on that request. On Android include Accept/Reject notification actions; iOS ignores them (tap opens the card).

Reuse what ticket 01 proved. See SPEC.md §2, §4.

**Blocked by:** 03 (Local CA + HTTPS), 10 (Live staff queue)

**Status:** done (manual phone checks pending; the expired push lands with ticket 14, which creates expiry)

- [ ] Push received with the app closed on iOS and Android for each event type (manual check)
- [x] Paused staff receive nothing (test with a fake push sender)
- [x] BN user gets BN text, EN user gets EN text (test)
- [x] 410 response removes the subscription (test)
- [ ] Android action buttons Accept/Reject work (manual check)

Notes: subscriptions belong to a session and are deleted on logout. Pushes go through a `push_outbox` table written in the same transaction as the request change. Only FCM, Apple, and Mozilla push endpoints are accepted. The VAPID key is `<data_dir>/vapid.pem`.
