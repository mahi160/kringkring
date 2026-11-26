# 08: Admin: menu + locations

**What to build:**

Admin manages:
- **Menu items**: EN + BN name, colour (from palette), icon (from a bundled curated icon set), sort order, active flag, and which modifiers apply (modifiers are admin-defined bilingual chips, e.g. Sugar, Milk).
- **Locations**: EN + BN name, sort order, active flag.
- Each employee's **default location** (on the user edit form from ticket 07).

Seed sensible defaults on first run (Tea, Raw tea, Black coffee, Water; Sugar, Milk) so the app is usable immediately. Deactivate instead of delete when referenced by requests.

See SPEC.md §5 Admin.

**Blocked by:** 07 (Admin: users)

**Status:** done

Removal is implemented as deactivation for all catalog entries. Stored IDs and records are retained for request history; request creation and history views are separate tickets.

- [x] CRUD + reorder works for items, modifiers, locations
- [x] Fresh DB contains the seeded defaults (test)
- [x] Deactivated items/locations disappear from employee-facing lists but history keeps them (test)
- [x] Employee default location saved and shown on the user edit form
