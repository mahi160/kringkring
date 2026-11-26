# 19: Admin: history + status

**What to build:**

Admin views: request history for the last 30 days (filter by status/user/date, newest first) and a live list of staff with on-duty / paused / online (SSE connected) status. Data older than 30 days is pruned daily.

See SPEC.md §5 Admin.

**Blocked by:** 12 (On duty)

**Status:** done

- [x] History shows all fields incl. reasons and actors
- [x] Status view updates live when staff pause or open/close the app
- [x] Prune removes rows older than 30 days only (test)
