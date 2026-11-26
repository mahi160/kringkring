(() => {
  const feed = document.querySelector("[data-sound-feed]");
  const toggle = document.querySelector("[data-sound-toggle]");
  const soundKey = "kringkring.muted";
  const player = window.AudioContext || window.webkitAudioContext;
  const readMuted = () => {
    try { return localStorage.getItem(soundKey) === "true"; }
    catch { return false; }
  };
  let muted = readMuted();
  let audio;
  let output;

  function syncMute() {
    toggle.setAttribute("aria-pressed", String(muted));
    if (output) output.gain.value = muted ? 0 : 1;
  }

  function unlock(event) {
    if (!event.isTrusted || !player || muted || audio?.state === "running") return;
    try {
      if (!audio) {
        audio = new player();
        output = audio.createGain();
        output.connect(audio.destination);
      }
      audio.resume().catch(() => {});
    } catch {
      // Audio is optional; order feedback remains visible.
    }
  }

  if (player) {
    syncMute();
    toggle.hidden = false;
    toggle.addEventListener("click", (event) => {
      muted = !muted;
      try { localStorage.setItem(soundKey, String(muted)); } catch {}
      syncMute();
      unlock(event);
    });
    window.addEventListener("storage", (event) => {
      if (event.key !== soundKey && event.key !== null) return;
      muted = readMuted();
      syncMute();
    });
    document.addEventListener("click", unlock, true);
    document.addEventListener("keydown", unlock, true);
  }

  let lastBell = -Infinity;
  function ring() {
    if (muted || document.visibilityState !== "visible" || audio?.state !== "running") return;
    const now = audio.currentTime;
    if (now - lastBell < 0.3) return;
    lastBell = now;
    for (const delay of [0, 0.15]) {
      for (const [frequency, volume, tail] of [[880, 0.10, 0.24], [2120, 0.03, 0.16]]) {
        const start = now + delay;
        const oscillator = audio.createOscillator();
        const gain = audio.createGain();
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(0.001, start);
        gain.gain.linearRampToValueAtTime(volume, start + 0.008);
        gain.gain.exponentialRampToValueAtTime(0.001, start + tail);
        oscillator.connect(gain).connect(output);
        oscillator.start(start);
        oscillator.stop(start + tail);
      }
    }
  }

  const snapshot = () => new Map(Array.from(
    feed.querySelectorAll("[data-request-id]"),
    (card) => [card.dataset.requestId, card.dataset.requestStatus],
  ));
  const isPaused = () => feed.querySelector("[data-duty-paused]")?.dataset.dutyPaused === "true";
  let before = snapshot();
  let paused = isPaused();
  feed.addEventListener("htmx:sseMessage", () => {
    const after = snapshot();
    const nowPaused = isPaused();
    const changed = [...after].some(([id, status]) =>
      feed.dataset.soundFeed === "staff"
        ? !paused && !nowPaused && status === "pending" && !before.has(id)
        : status !== "pending" && before.get(id) !== status,
    );
    before = after;
    paused = nowPaused;
    if (changed) ring();
  });

  if (feed.dataset.soundFeed === "employee") {
    const canVibrate = /Android/i.test(navigator.userAgent) && typeof navigator.vibrate === "function";
    if (canVibrate) {
      document.addEventListener("click", (event) => {
        if (event.isTrusted && event.target.closest(".order-grid .tile")) navigator.vibrate(12);
      });
      document.addEventListener("submit", (event) => {
        if (event.isTrusted && event.target.matches("[data-order-form]")) navigator.vibrate(30);
      }, true);
    }

    // Keep the gesture-unlocked audio context on the order page when sending.
    // SSE swaps fire these events without a target.
    document.addEventListener("htmx:beforeSwap", ({ detail }) => {
      if (detail.target?.id !== "order-controls") return;
      if ([409, 422, 503].includes(detail.xhr.status)) {
        detail.shouldSwap = true;
        detail.isError = false;
      }
    });
    document.addEventListener("htmx:afterSwap", ({ detail }) => {
      if (detail.target?.id !== "order-controls") return;
      if (detail.xhr.status === 200) {
        history.replaceState(history.state, "", "/employee/menu#requests");
        document.querySelector("#requests").focus();
      } else {
        document.querySelector("#compose").focus();
      }
    });
  }
})();
