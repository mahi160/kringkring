# 06: Bengali + English

**What to build:**

All UI strings come from one catalog per language (EN, BN); no hard-coded UI text in templates. Language resolution: device override (cookie) > user default > Bengali. Header gets a বাং/EN toggle next to the theme toggle. Helper to render numbers/times with Bengali digits in BN mode. PIN inputs/keypads always use 0–9. Provide a helper for admin-entered bilingual content: show the current language, fall back to the other if empty.

See SPEC.md §7.

**Blocked by:** 04 (Users, PIN login, sessions), 05 (Metro design system)

**Status:** done

- [x] Toggle switches every visible string on existing pages (login, health/styleguide) without reload glitches
- [x] A test fails if a catalog key exists in one language but not the other
- [x] Bengali digit helper unit-tested (e.g. 12:05 → ১২:০৫)
- [x] Bilingual fallback helper unit-tested (BN empty → EN shown, and vice versa)
