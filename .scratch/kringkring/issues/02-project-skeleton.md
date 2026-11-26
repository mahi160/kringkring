# 02: Project skeleton

**What to build:**

Create the real Rust project for **kringkring**. Running `kringkring serve` starts an axum server on a configurable port (plain HTTP for now), creates the data dir (default `/var/lib/kringkring`, overridable by flag/env for dev), opens SQLite in WAL mode with versioned migrations, and serves a health page. All static assets are embedded in the binary (rust-embed or include_dir). Add askama templates and vendored htmx. Provide a documented command to build a static musl binary.

Keep it minimal: no auth, no UI design yet. See SPEC.md §2, §9.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `cargo build` and `cargo test` pass; `cargo clippy -- -D warnings` clean
- [x] `kringkring serve --data-dir ./tmpdata` creates the dir and DB, `GET /health` returns 200 with DB check
- [x] Migrations run once and are idempotent across restarts (test)
- [x] An embedded static file is served (e.g. htmx) with no files on disk next to the binary
- [x] musl build command documented in README and produces a static binary (`file` shows statically linked)
