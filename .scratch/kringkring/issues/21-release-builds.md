# 21: Release builds

**What to build:**

GitHub Actions workflow: on tag `v*`, run tests, build static musl binaries for x86_64 and aarch64, attach them (plus SHA256 sums) to a GitHub Release with predictable asset names that install.sh can fetch as "latest".

See SPEC.md §2, §9.

**Blocked by:** 02 (Project skeleton)

**Status:** implemented; GitHub tag-push validation pending

`actionlint` and ShellCheck passed. Both local release artifacts were statically linked, passed `/health` in clean Alpine, and matched their SHA256 checksums. The workflow has not run on GitHub.

- [x] CI is configured to run fmt, clippy, tests on every push/PR
- [ ] Tag push produces a release with both binaries and checksums
- [x] Packaged amd64 binary runs in a clean Alpine container; download verification awaits publication
