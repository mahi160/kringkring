# 04: Users, PIN login, sessions

**What to build:**

Add users with role `employee` | `staff` plus an `is_admin` flag, username (unique), 4-digit PIN hashed with argon2, default language, default location (nullable, used later).

`kringkring init --username X --pin 1234` creates the first admin (refuses if any admin exists). A plain username + PIN login form (replaced later by ticket 17) creates a session cookie (HttpOnly, Secure, SameSite=Lax) that lasts until logout or revocation. 5 consecutive wrong PINs lock the account. Middleware guards routes by role/admin. Logout destroys the session.

See SPEC.md §3.

**Blocked by:** 02 (Project skeleton)

**Status:** done

- [x] `init` creates an admin; second `init` refuses (test)
- [x] Correct PIN logs in; wrong PIN increments counter; 5th failure locks; locked user cannot log in even with correct PIN (tests)
- [x] Session survives server restart; logout and revoke invalidate it (tests)
- [x] Employee cannot reach staff/admin routes and vice versa (tests)
- [x] PIN never stored or logged in plaintext
