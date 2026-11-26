# 05: Metro design system

**What to build:**

Use the **frontend-design** skill. Build the shared look: modern Metro — flat solid-colour tiles, ~10-colour palette, white line icons, label bottom-left, large light headings, section titles bleeding off the edge, 0–2px radius, high contrast, dark default + light theme. Embed subsetted Inter and Noto Sans Bengali (works offline). Base layout template with a header containing the ☾/☀ theme toggle (persists per device, defaults to system). A tile component with a press-tilt effect. Mobile-first; tap targets ≥ 48px.

Deliver a hidden `/styleguide` page showing every component. See SPEC.md §6.

**Blocked by:** 02 (Project skeleton)

**Status:** done

Verified in Chromium: both themes, persistence, embedded Latin/Bengali fonts with external network requests blocked, and no overflow at 320px. Lighthouse accessibility: 100.

- [x] `/styleguide` renders tiles in all palette colours, headings, buttons, chips, cards, in dark and light
- [x] Theme toggle persists across reload; first visit follows system preference
- [x] Fonts load from the binary with network disabled; Bengali sample text renders correctly
- [x] No layout overflow at 320px width; Lighthouse accessibility ≥ 90 on `/styleguide`
