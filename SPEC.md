# ক্রিং ক্রিং — Spec

A LAN-only request/notification app. Employees tap a tile ("Tea", "Black coffee", …); on-duty staff get an instant push on their phone and accept, reject, or complete it. Ships as one static binary plus a Proxmox install script.

Display the brand as **ক্রিং ক্রিং** in every language. Keep `kringkring` as the slug for the binary, service, data directory, and container hostname.

## 1. Goals and constraints

- UX first: as few taps as possible, big targets, no extra steps.
- Instant, best-effort-reliable delivery to iOS and Android PWAs.
- Single static binary that runs anywhere; the smallest LXC that works.
- Local network only. No domain and no third-party DNS. Outbound internet is available (required for push).
- About 30 users at one site today.
- Languages: English and Bengali, both first-class.

## 2. Stack

- Rust: `axum`, `askama` templates, `htmx`, `rusqlite` (bundled SQLite), `web-push` (VAPID).
- Live updates while the app is open: Server-Sent Events.
- Background delivery when the app is closed: Web Push through APNs (iOS) and FCM (Android).
- Static musl builds for amd64 and arm64. All assets (CSS, JS, icons, fonts, sounds) are embedded in the binary.
- Fonts are subsetted and embedded so the app works offline: Inter (Latin) and Noto Sans Bengali.
- The UI is built with the `frontend-design` skill.

## 3. Roles and auth

- Each user has a role, either **employee** or **staff**. **Admin** is an extra flag that can be set on either role.
- Credentials are a unique username and a 4-digit numeric PIN, hashed with argon2.
- A session cookie (HttpOnly, Secure, SameSite=Lax) lasts until the user logs out or an admin revokes it.
- After 5 failed PIN attempts the account locks until an admin unlocks it or resets the PIN.
- The first admin is created by `install.sh`, which runs `kringkring init`. The web app has no setup or first-run page.

### Onboarding (main path): personal invite QR

1. The admin opens a user and shows that user's invite QR. It is single-use and expires after 24 hours.
2. The QR points to `http://<ip>/invite/<token>`. Plain HTTP is only needed here because the CA isn't trusted yet.
3. A guided flow ("Step N of 4") walks the user through:
   1. Installing the CA, with separate iOS and Android instructions and screenshots.
   2. Opening the HTTPS URL and adding the app to the Home Screen.
   3. Logging in with the username prefilled; the user sets or enters their PIN.
   4. Enabling notifications.

### Re-login (fallback)

- An iOS-style scroll wheel of names, sorted alphabetically with a snapping, highlighted centre row. Selecting a name opens a large PIN keypad.
- The device remembers its last user, who is pre-centred on the wheel.
- A search icon filters the wheel.

## 4. Request lifecycle

```
pending ──accept──▶ accepted ──done──▶ done
   │                   │
   ├─reject(reason)──▶ rejected
   ├─cancel──────────▶ cancelled          (employee, no reason, 3 s undo)
   └─timeout─────────▶ expired
                       accepted ──cancel(reason)──▶ cancelled   (employee)
```

- Every open request is broadcast to all on-duty staff. The first staff member to accept claims it.
- Other staff keep seeing the card, greyed out and disabled, labelled "Accepted by X".
- An employee can have at most 3 open requests (pending or accepted). Each request is a separate card, with no cart.
- **Escalation:** a pending request is re-pushed to all on-duty staff every 60 s. It expires after 10 min. Both times are configurable by the admin.
- **No staff available:** if no staff member is on duty when the request is sent, the employee sees this immediately.
- **Reasons:** staff reject reasons and employee cancel-after-accept reasons are admin-defined presets, plus "Other…" for free text.
- **Employee pushes:** the employee gets a push on accepted, rejected, done, and expired.
- **Staff pushes:** staff get a push for every new request, for each re-push, and when an employee cancels.

### On duty

- A staff member is on duty while logged in.
- A **Pause** toggle at the top of the staff screen takes them off duty temporarily.
- Logging out also takes them off duty.

## 5. Screens

### Employee

- A grid of Metro-style tiles for the menu items, plus a **Custom** tile that opens a short free-text field.
- **Ordering takes 2 taps.** Tapping a tile selects it and shows the following on the same screen:
  - Modifier chips such as sugar and milk.
  - A row of location chips with the employee's default location pre-selected.
  - A big **Send** button.
- Live status cards for open requests, with Cancel (and a reason picker once the request is accepted).

### Staff

- One live queue, oldest request first.
- Each card shows the item, its modifiers, the location (large), who asked, and a running timer.
- Pending cards have **Accept** and **Reject** buttons. Cards you accepted have a **Done** button.
- Cards accepted by others are greyed out and show who took them.
- The Pause toggle sits in the header.

### Admin

- Users: create, edit, reset PIN, unlock, revoke sessions, show invite QR, and see who is on duty and online.
- Menu items: EN and BN names, a colour from the palette, an icon from the bundled set, sort order, and modifiers.
- Locations: EN and BN names. Each employee has a default location.
- Reason presets: EN and BN, for both the staff list and the employee list.
- Settings: re-push interval and expiry time.
- History: the last 30 days.
- CA download page with a QR code.

### Global header

- Language toggle (বাং/EN) and theme toggle (☾/☀), both saved per device. The theme defaults to the system setting.

### Push nag

- If the app is open in a browser tab, a full-screen "Add to Home Screen" guide is shown.
- In the installed PWA, a persistent banner prompts the user to enable notifications until they do.

## 6. Visual direction

Modern Metro:

- Flat tiles in solid colours, about 10 palette colours, with white line icons and labels in the bottom-left corner.
- Large light headings, and section titles that run off the edge of the screen.
- Corners at 0–2 px radius.
- High contrast, with a dark theme by default.
- **Press feedback:** the tile tilts when pressed. On Android it also gives haptic feedback.
- No decorative animation; only state changes animate.
- **Sound:** the app plays a "kring kring" bell when it is open. Web push uses the OS default sound, because custom sounds aren't supported for web push on iOS or Android.

## 7. i18n

- Each user's language is set per user, defaulting to Bengali. Each device can override it.
- All UI strings live in one catalog per language.
- Admin-entered content has both EN and BN fields. If one is empty, the other is shown.
- Push text is written in the recipient's language.
- Timers and times use Bengali digits in Bengali mode. The PIN keypad always uses 0–9.

## 8. Network and TLS

- The app is `https://kringkring.questionpro.lan`. The office gateway container (dnsmasq + Caddy) resolves `*.questionpro.lan`, terminates HTTPS with the office CA, and proxies to plain HTTP on port 80 of the container named `kringkring`.
- Phones trust the office CA once, following [Trust the office certificate](https://hub.questionpro.lan/announcements/trust-the-office-certificate-for-questionprolan). The app has no certificates of its own.
- The origin depends only on the name, so the container's DHCP address can change.
- Outbound internet is required to reach the APNs and FCM push services.

## 9. Data and ops

- All data lives in `/var/lib/kringkring/`: the SQLite database (WAL mode), and the VAPID key (mode 600).
- A nightly `VACUUM INTO` snapshot is written, and the last 7 days are kept.
- `install.sh` runs on the Proxmox host. It:
  - Creates an unprivileged Alpine LXC named `kringkring` with 1 core, 64 MB RAM, a 1 GiB rootfs, and DHCP.
  - Mounts a separate 1 GiB volume at `/var/lib/kringkring` with `backup=1`, so the container can be rebuilt without losing data.
  - Installs an OpenRC service that runs as the `kringkring` user with only `CAP_NET_BIND_SERVICE`.
  - Prompts for the first admin's username and PIN, then runs `kringkring init`.
- `install.sh upgrade CTID` converts an existing container in place, keeping its data.
- Each push to `main` publishes a build on GitHub. The container checks every 5 minutes, installs a new build after verifying its SHA-256, and rolls back if `/health` fails.

## 10. Risks and day-1 spike

1. **Unverified:** whether iOS Web Push works for a Home Screen PWA served with a user-installed CA certificate. The [minimal spike and device runbook](.scratch/kringkring/push-spike/README.md) are ready; Android Chrome delivered a test notification without reopening the demo, but standalone installation failed: the Home Screen shortcut opens a Chrome tab. OS/browser versions and exact CA installation steps are not yet recorded. The real iPhone (iOS 16.4+) result is pending. Record the result, OS/browser versions, exact install steps that worked, and gotchas here and in [ticket 01](.scratch/kringkring/issues/01-ios-android-push-spike.md). If iOS fails, stop and revisit the TLS design. If both pass, archive the spike and proceed to ticket 03 (Local CA + HTTPS); the plain-HTTP skeleton from ticket 02 is complete.
2. Web Push is best-effort. iOS may delay pushes in Focus or Low Power mode. Re-push and expiry reduce the impact but can't remove it.
3. If the static IP collides with another device, or the IP has to change, every phone needs onboarding again.

## Out of scope for v1

Stats and charts, quantities, multiple sites, Pushover or native apps, and a station display.
