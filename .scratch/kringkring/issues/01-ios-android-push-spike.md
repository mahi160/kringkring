# 01: iOS/Android push spike

**What to build:**

Prove the riskiest assumption before anything else: a Home Screen PWA served over HTTPS with a **user-installed CA** can receive Web Push on a real iPhone (iOS 16.4+) and a real Android phone (Chrome).

Build a throwaway Rust program (separate folder, deleted or archived afterwards) that:
- generates a root CA and a server cert for the machine's LAN IP (rcgen or equivalent),
- serves the CA file over plain HTTP and a tiny PWA (manifest + service worker + one "Subscribe" button + one "Send test push" button) over HTTPS,
- generates VAPID keys and sends a push to stored subscriptions.

A human must run the device test. Record the result (works / fails, iOS version, Android version, any gotchas) in the ticket and in SPEC.md §10. If iOS fails, stop and report: the TLS design must change.

See SPEC.md §8, §10.

**Blocked by:** None (can start immediately)

**Status:** Android browser push confirmed; Android standalone install failed; iPhone validation pending

Run the [push spike](../push-spike/README.md) on a private LAN. Test iOS first and stop if it fails; do not proceed to the local-CA implementation until the device results are recorded.

| Device | OS / browser version | CA and warning-free HTTPS | Home Screen permission | Closed-app delivery | Gotchas |
| --- | --- | --- | --- | --- | --- |
| iPhone | Pending | Not tested | Not tested | Not tested | — |
| Android | Versions not reported | User reported the HTTPS demo working | Chrome subscription accepted; no standalone install | User confirmed notification without reopening the demo | Install reports “This app cannot be installed”; Add to Home screen creates a shortcut that opens a Chrome tab. Adding 192/512 PNG icons did not fix it; Chrome's installability error is not yet captured. Exact CA steps not reported. |

- [ ] CA installs and is fully trusted on the iPhone; HTTPS page loads without warning
- [ ] PWA added to Home Screen on iOS; notification permission granted from a button tap
- [ ] Push arrives on iOS with the app **closed**
- [x] Android Chrome receives a notification without reopening the demo
- [ ] Android standalone install and closed-app push verified together
- [ ] Findings written into SPEC.md §10 (exact install steps that worked per OS)
