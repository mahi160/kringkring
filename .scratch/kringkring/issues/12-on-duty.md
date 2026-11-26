# 12: On duty

**What to build:**

A staff user is on duty while they have an active session and are not paused. Staff header gets a big **Pause** toggle. Logout = off duty. When an employee sends and zero staff are on duty, the request is not created and the employee immediately sees "No staff available".

See SPEC.md §4 On duty.

**Blocked by:** 10 (Live staff queue)

**Status:** done

- [x] Pause/unpause persists and is reflected immediately
- [x] With all staff paused or logged out, Send shows 'No staff available' and creates nothing (test)
- [x] With ≥1 on-duty staff, Send works as before (test)
