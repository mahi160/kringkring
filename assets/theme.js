(() => {
  const key = "kringkring.theme";
  const root = document.documentElement;
  const system = matchMedia("(prefers-color-scheme: light)");
  try {
    const saved = localStorage.getItem(key);
    if (saved === "light" || saved === "dark") root.dataset.theme = saved;
  } catch {
    // The system theme still works if this browser blocks storage.
  }

  document.addEventListener("DOMContentLoaded", () => {
    const toggle = document.querySelector("[data-theme-toggle]");
    const current = () => root.dataset.theme ?? (system.matches ? "light" : "dark");
    const sync = () => toggle.setAttribute("aria-pressed", String(current() === "dark"));
    sync();
    toggle.hidden = false;
    system.addEventListener("change", sync);
    toggle.addEventListener("click", () => {
      const next = current() === "dark" ? "light" : "dark";
      root.dataset.theme = next;
      try {
        localStorage.setItem(key, next);
      } catch {
        // Still switch for the current page.
      }
      sync();
    });
  });
})();
