# 17: Name-wheel login

**What to build:**

Replace the plain login form: an iOS-style scroll wheel of active user names (alphabetical, scroll-snap, big highlighted centre row), then a large PIN keypad (0–9). The device remembers the last user (cookie/localStorage) and pre-centres them. A search icon filters the wheel. Must feel good at 30+ users and work with keyboard/screen readers.

Use the frontend-design skill. See SPEC.md §3 Re-login.

**Blocked by:** 04 (Users, PIN login, sessions), 05 (Metro design system)

**Status:** implemented; VoiceOver/TalkBack verification pending

Chromium verified keyboard navigation, search, keypad, and remembered-name centering with 33 active users at 360px.

- [x] Last user pre-centred on return visit
- [x] Search narrows the wheel; selecting opens the keypad
- [x] Login, lockout, and error states identical to ticket 04 behaviour (existing tests still pass)
- [ ] Works with VoiceOver/TalkBack and keyboard (manual)
