# ক্রিং ক্রিং

## Local development

```sh
cargo run --locked -- serve --data-dir ./tmpdata --port 8080
curl -fsS http://127.0.0.1:8080/health
```

`serve` listens over plain HTTP on `--host` (`KRINGKRING_HOST`, default `127.0.0.1`) and `--port` (`KRINGKRING_PORT`, default `8080`). It creates `kringkring.sqlite3` in the data directory, enables WAL, and applies pending migrations before listening. The default data directory is `/var/lib/kringkring`; set `KRINGKRING_DATA_DIR` to override it. Explicit flags take precedence over environment variables. `--port 0` chooses a free port, printed on startup.

`/health` returns 200 when SQLite is readable and 503 otherwise. The Askama template and `/assets/htmx.min.js` are embedded; no templates or asset files are required beside the executable. htmx 2.0.11 is vendored from [the upstream distribution](https://cdn.jsdelivr.net/npm/htmx.org@2.0.11/dist/htmx.min.js) with its [license](assets/htmx.LICENSE.txt).

## Office HTTPS

In the office the app is `https://kringkring.questionpro.lan`. The office gateway container resolves the name, terminates HTTPS with the office CA, and proxies to port 80 of the container named `kringkring` (see `../office-hub/infra/README.md`). Each phone trusts the office certificate once, following [Trust the office certificate](https://hub.questionpro.lan/announcements/trust-the-office-certificate-for-questionprolan). The app itself serves plain HTTP and has no certificates of its own.

## Web Push

`serve` creates a VAPID key at `<data-dir>/vapid.pem` (mode 600) on first run. Keep it with the data: replacing it invalidates every phone's subscription. Set `--push-contact` (`KRINGKRING_PUSH_CONTACT`) to a real `mailto:` address or `https:` URL; push services can reject unreachable contacts. The staff queue and employee menu offer **Turn on notifications**. A subscription belongs to the signed-in device and is removed on logout. On-duty staff are notified of new requests and cancellations; employees are notified when their request is accepted, rejected, or delivered. Notifications use the device's language. On Android, new-request notifications have Accept and Reject buttons. Push is best-effort; the live queue works without it.

## Accounts and sessions

Initialize the first admin in the same data directory used by `serve`:

```sh
kringkring init --data-dir ./tmpdata --username <username> --pin <four-digit-pin>
```

The CLI accepts the PIN as an argument, so it can be visible in shell history or the process list. Do not log the command or run it on an untrusted host. The database stores only an Argon2id hash. The initial admin has the employee role; admin permission is independent of role.

For local development, open `http://localhost:8080/login` in a browser that permits Secure cookies on localhost. Do not disable the cookie's Secure flag; use HTTPS in the office (see [Office HTTPS](#office-https)). Choose your name on the scroll wheel (or use **Search names**) and enter your PIN. The browser remembers and centers your last successful account. Five consecutive wrong PINs lock the account. An administrator can unlock the account or reset its PIN.

After login, `/account` offers logout. Administrators can open **Manage users** to add or edit users, reset PINs, unlock accounts, revoke sessions, or deactivate accounts. Resetting a PIN or deactivating a user revokes all their sessions. The last active administrator cannot be demoted or deactivated. Administrators can revoke all sessions for a username with a same-origin form POST to `/admin/sessions/revoke` (field `username`). Server-side sessions survive restart and have no timer; browsers can expire or clear their cookies.

## Menu and locations

From the admin account page, open **Manage menu and locations**. Set smaller sort-order numbers to show entries earlier. Uncheck **Active** to hide an entry without deleting its record; check it again to restore it. Set an employee's default location under **Manage users**. Employees can order at `/employee/menu`. Add at least one active location first. For a two-tap order, give the employee a default location, then tap an item and **Send request**. Pending and accepted requests count toward the limit of three open requests per employee.

## Live staff queue

Sign in as staff and open **Staff queue** at `/staff`. The queue is oldest first. The first staff member to accept a request owns it; only that person can mark it done. Staff and employee cards update over SSE using the embedded [htmx SSE extension 2.2.4](https://htmx.org/extensions/sse/) ([license](assets/htmx-sse.LICENSE.txt)). The stream reloads current state after reconnecting. Staff are on duty when they have an active session and are not paused. Pause applies across the account’s devices; paused staff can still finish requests they accepted. If nobody is on duty, Send shows an error and creates no request.

## Reject and cancel reasons

Manage the two bilingual preset lists under [**Menu and locations**](http://localhost:8080/admin/catalog). Staff can reject a pending request with a preset or their own reason. Employees can undo a pending cancellation within three seconds; cancelling an accepted request requires a reason. Reasons appear on the other party’s live card.

## In-app feedback

Tap or press a key on the open staff queue or employee menu to let the browser play the in-app bell. Use the speaker button to mute it on this browser. Supported Android browsers also vibrate on menu tile taps and Send. System push sounds are unaffected.

## Admin history and status

From the admin account page, open **Request history** (`/admin/history`) or **Staff status** (`/admin/status`). History filters use UTC dates; requests older than 30 days are pruned at startup and daily thereafter. On the live staff list, **Online** means a queue stream is connected, while **On duty** means the staff member has an active session and is not paused.

## Language

The header language button saves a preference for the current browser. Without it, the app follows the signed-in user's preference or defaults to Bengali. The brand stays **ক্রিং ক্রিং** in both languages; PINs always use ASCII digits.

## Metro styleguide

Open [`/styleguide`](http://localhost:8080/styleguide) to preview the tile palette, type, buttons, cards, and status chips. The header toggle overrides the system theme for that browser. The Inter Latin and Noto Sans Bengali WOFF2 subsets are embedded in `assets/fonts.css`, from Fontsource 5.3.0; their OFL licenses are in [`assets/fonts/`](assets/fonts/). No internet connection is needed, but the app server must be reachable.

## Nightly backups

`serve` takes a SQLite snapshot at 02:00 UTC using `VACUUM INTO`. On startup, it catches up the latest missed slot; it does not recreate earlier nights from current data. Snapshots are stored in `<data-dir>/backups/kringkring-YYYY-MM-DD.sqlite3`. The newest seven completed snapshots are kept. The backup runs on a separate SQLite connection and leaves the server accepting requests.

Snapshots contain application data, PIN hashes, and session records. Keep the backup directory private and copy backups to separate storage if you need protection from disk failure. To test a restore, copy one snapshot to `kringkring.sqlite3` in a **new, empty, private** data directory, then start `serve` with that directory on a separate loopback port. Do not overwrite a database while its server is running.

## Proxmox install and automatic updates

Every push to `main` runs CI (formatting, Clippy, tests), builds the static x86_64 binary in Alpine, and republishes it with its checksum as the `kringkring-build` prerelease. The container runs `/usr/local/sbin/kringkring-update` from cron every 5 minutes. It downloads the 100-byte checksum file and does nothing if that build is already installed. Otherwise it downloads the binary, checks the SHA-256, restarts the service, and waits for `/health`. If the new build fails, it restores the previous binary and skips that build until the next push. GitHub never connects to the office. Log: `/var/log/kringkring-update.log` in the container.

Set up a container once with `bash deploy.sh` from your laptop. It connects to `root@qpbd.local` (you type the host password; no guest SSH keys), copies `infra/install.sh` and `infra/kringkring-update`, and runs the installer:

- `bash deploy.sh` creates an unprivileged Alpine LXC named `kringkring` (1 core, 64 MB RAM, 1 GiB rootfs, DHCP) with a separate 1 GiB `/var/lib/kringkring` volume (`backup=1`). It asks for the CTID, storage, bridge, push contact email, and the first admin's username and PIN. It refuses an existing CTID or a second container named `kringkring`.
- `bash deploy.sh upgrade CTID` converts an existing container in place, keeping its data: it renames it to `kringkring`, adds a DHCP network interface if it has none, and reboots it.

Both install the `kringkring` OpenRC service (runs as the `kringkring` user with only `CAP_NET_BIND_SERVICE`, logs to `/var/log/kringkring.log`), install the updater, and install the latest build immediately. A build must be published first. The office gateway picks up the container name within a minute.

## Static Linux builds and releases

Run from the project root with Docker, `file`, and `sha256sum` available:

```sh
sh scripts/build-release.sh x86_64-unknown-linux-musl
sh scripts/build-release.sh aarch64-unknown-linux-musl
```

Docker needs emulation to build for a different architecture. The script checks static linking, starts the binary in clean Alpine without internet access, and verifies `/health` before writing the binary and checksum to `dist/`.

GitHub Actions runs formatting, Clippy, and tests on every push and pull request. Pushing a `v*` tag (for example, `v0.1.0`) also builds both architectures and publishes the release as **Latest** with these assets:

- `kringkring-x86_64-unknown-linux-musl` and `kringkring-x86_64-unknown-linux-musl.sha256`
- `kringkring-aarch64-unknown-linux-musl` and `kringkring-aarch64-unknown-linux-musl.sha256`

Download the binary and matching checksum to the same directory. Run `sha256sum -c <binary>.sha256` before making the binary executable and running it.
