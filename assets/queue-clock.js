(() => {
  const digits = new Intl.NumberFormat(document.documentElement.lang, {
    minimumIntegerDigits: 2,
    useGrouping: false,
  });

  function tick() {
    const now = Math.floor(Date.now() / 1000);
    for (const timer of document.querySelectorAll("[data-created]")) {
      const elapsed = Math.max(0, Number(timer.dataset.closed ?? now) - Number(timer.dataset.created));
      const formatted = digits.format(Math.floor(elapsed / 60)) + ":" + digits.format(elapsed % 60);
      if (timer.textContent !== formatted) timer.textContent = formatted;
    }
  }

  tick();
  document.addEventListener("htmx:sseMessage", tick);
  setInterval(tick, 1000);
})();
