# 07: Admin: users

**What to build:**

Admin screen to manage users: list, create (username, role, admin flag, PIN, default language), edit, reset PIN, unlock, revoke all sessions, deactivate. Admins cannot remove the last admin. Uses the Metro design system and i18n catalog.

See SPEC.md §3, §5 Admin.

**Blocked by:** 06 (Bengali + English)

**Status:** done

- [x] Admin can create employee, staff, and staff+admin users who can then log in
- [x] Reset PIN + unlock works; revoke logs the user out on their next request
- [x] Removing/demoting the last admin is refused (test)
- [x] Non-admins get 403 on all admin routes (test)
- [x] Usable at 360px width; all strings in both languages
