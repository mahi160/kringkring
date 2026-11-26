# 20: Nightly backups

**What to build:**

Once per night write a `VACUUM INTO` snapshot of the DB to a backups folder in the data dir; keep the newest 7, delete older. Must not block requests noticeably.

See SPEC.md §9.

**Blocked by:** 02 (Project skeleton)

**Status:** done

Scheduled for 02:00 UTC with catch-up on startup. The smoke test ran the actual server against a temporary data directory and confirmed the latest snapshot opens and is not overwritten on same-night restart.

- [x] Snapshot is a valid, openable SQLite DB (test)
- [x] Rotation keeps exactly 7 (test)
- [x] Missed run after downtime happens on next start
