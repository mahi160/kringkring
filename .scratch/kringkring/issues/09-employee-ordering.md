# 09: Employee ordering

**What to build:**

Employee home: grid of active menu tiles plus a **Custom** tile. Tap a tile → it is selected and, on the same screen, modifier chips, a location chip row (employee's default pre-selected) and a big **Send** appear. Custom opens a short free-text field (max ~80 chars) in the same panel. Send creates a `pending` request and shows a status card on the same screen. Max 3 open (pending or accepted) requests per employee; the 4th Send is refused with a clear message.

Request row stores: employee, item (or custom text), modifiers, location, status, timestamps, actor ids. Use this state machine (later tickets add transitions):

```
pending → accepted → done
pending → rejected | cancelled | expired
accepted → cancelled
```

See SPEC.md §4, §5 Employee.

**Blocked by:** 08 (Admin: menu + locations)

**Status:** done

Employee requests are stored and shown on the same screen. Live staff updates and mutation endpoints for later transitions are separate tickets.

- [x] Two taps (tile, Send) create a request with default location (test via HTTP)
- [x] Modifiers and location change are stored
- [x] Custom text request works; empty custom text refused
- [x] 4th open request refused (test)
- [x] Invalid state transitions rejected by a single domain function (unit tests)
