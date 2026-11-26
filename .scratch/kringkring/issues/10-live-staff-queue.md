# 10: Live staff queue

**What to build:**

Staff home: one live queue, oldest first. Card shows item, modifiers, location (large), requester, running timer. Pending: **Accept** / **Reject** (plain reject for now; reasons come in 11). Your accepted cards: **Done**. Cards accepted by another staff stay visible greyed/disabled with "Accepted by X".

Live updates via SSE for both staff queue and employee status cards (htmx SSE extension). First accept wins: the accept must be atomic in SQLite (conditional UPDATE on status = pending); the loser sees the greyed card. Done/terminal cards leave the queue after a short delay.

See SPEC.md §4, §5 Staff.

**Blocked by:** 09 (Employee ordering)

**Status:** done

Verified in Chromium with two staff sessions and one employee session. Both staff views updated in 19 ms on loopback; the stream reconnected after a real server restart.

- [x] New request appears on all open staff screens within 1s without reload
- [x] Two staff accepting simultaneously: exactly one wins (concurrency test)
- [x] Other staff see the card greyed with the accepter's name
- [x] Employee status card updates live on accept/reject/done
- [x] SSE reconnects after server restart and re-renders current state
