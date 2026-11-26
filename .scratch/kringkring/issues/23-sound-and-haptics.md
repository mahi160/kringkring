# 23: Sound + haptics

**What to build:**

In-app "kring kring" bell sound when a new request arrives on an open staff screen and when an employee's request changes state (respect a per-device mute toggle; unlock audio on first user tap). Tile press tilt already exists; add vibration on Android (navigator.vibrate) for tile press and Send. Note: OS push uses the default system sound — do not try to customise it.

See SPEC.md §6.

**Blocked by:** 10 (Live staff queue)

**Status:** implemented; real-phone verification pending

Chromium verified chime scheduling and per-browser mute persistence. Android/iOS haptic branching was exercised using emulated user agents; physical vibration and audible playback still need real-device checks.

- [ ] Open staff screen plays the bell on a new request (manual, iOS + Android)
- [x] Mute toggle persists per device
- [ ] Android vibrates on Send; iOS silently skips
