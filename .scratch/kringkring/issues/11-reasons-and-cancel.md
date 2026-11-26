# 11: Reasons + cancel

**What to build:**

Admin manages bilingual reason presets in two lists: **staff reject** reasons and **employee cancel** reasons. Staff Reject opens a one-tap preset picker + "Other…" free text. Employee can cancel a **pending** request with one tap and a 3-second Undo (no reason). Employee cancelling an **accepted** request picks a reason (preset or Other). Reasons are shown on the other party's card.

See SPEC.md §4.

**Blocked by:** 10 (Live staff queue)

**Status:** done

- [x] Reject with preset and with free text both stored and shown to employee
- [x] Pending cancel + Undo within 3s restores the request; after 3s it is cancelled (test)
- [x] Accepted cancel requires a reason; staff card shows it live
- [x] Admin CRUD for both preset lists, bilingual
