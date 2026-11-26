# 14: Re-push + expiry

**What to build:**

A background task: every pending request is re-pushed to on-duty staff every N seconds (default 60) and moves to `expired` after M minutes (default 10); the employee is notified. Admin settings page to change N and M. Task must survive restarts (state derived from DB timestamps, not memory).

See SPEC.md §4.

**Blocked by:** 12 (On duty), 13 (Web Push)

**Status:** ready-for-agent

- [ ] Re-push fires at the configured interval while pending, stops once accepted (test with controllable clock)
- [ ] Request expires at M minutes; employee gets push + live card update (test)
- [ ] Restart mid-wait does not lose or duplicate expiry (test)
- [ ] Admin can change N and M; new values apply without restart
