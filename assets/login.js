(() => {
  const wheel = document.querySelector("[data-wheel]");
  if (wheel) {
    const search = document.querySelector("#login-search");
    const disclosure = document.querySelector("[data-search]");
    const empty = document.querySelector("[data-no-matches]");
    const locale = document.documentElement.lang;
    const collator = new Intl.Collator(locale, { sensitivity: "base" });
    const rows = Array.from(wheel.children).sort((a, b) => collator.compare(a.textContent.trim(), b.textContent.trim()));
    wheel.append(...rows);
    let visible = rows;
    let centered;
    const buttonFor = (row) => row.firstElementChild;
    const activeRow = () => document.activeElement?.closest(".name-wheel li");

    function paint(row, tabRow = activeRow() ?? row) {
      if (centered) centered.removeAttribute("data-centered");
      centered = row;
      if (row) row.setAttribute("data-centered", "");
      for (const entry of visible) buttonFor(entry).tabIndex = entry === tabRow ? 0 : -1;
    }

    function center(row) {
      if (!row) return;
      wheel.scrollTop = row.offsetTop - (wheel.clientHeight - row.offsetHeight) / 2;
      paint(row);
    }

    center(rows.find((row) => buttonFor(row).hasAttribute("data-last-user")) ?? rows[0]);
    if ("onscrollsnapchange" in wheel) {
      wheel.addEventListener("scrollsnapchange", (event) => {
        if (event.snapTargetBlock) paint(event.snapTargetBlock);
      });
    } else {
      let pending = false;
      wheel.addEventListener("scroll", () => {
        if (pending) return;
        pending = true;
        requestAnimationFrame(() => {
          pending = false;
          const axis = wheel.scrollTop + wheel.clientHeight / 2;
          const distance = (row) => Math.abs(row.offsetTop + row.offsetHeight / 2 - axis);
          paint(visible.reduce((closest, row) => !closest || distance(row) < distance(closest) ? row : closest, null));
        });
      }, { passive: true });
    }

    wheel.addEventListener("keydown", (event) => {
      const current = visible.indexOf(activeRow());
      if (current < 0) return;
      const next = {
        ArrowUp: Math.max(0, current - 1),
        ArrowDown: Math.min(visible.length - 1, current + 1),
        Home: 0,
        End: visible.length - 1,
      }[event.key];
      if (next === undefined) return;
      event.preventDefault();
      const row = visible[next];
      buttonFor(row).focus({ preventScroll: true });
      center(row);
    });

    disclosure.hidden = false;
    disclosure.addEventListener("toggle", () => {
      if (disclosure.open) search.focus();
    });
    search.addEventListener("input", () => {
      const needle = search.value.trim().normalize().toLocaleLowerCase(locale);
      visible = rows.filter((row) => {
        row.hidden = !buttonFor(row).textContent.trim().normalize().toLocaleLowerCase(locale).includes(needle);
        return !row.hidden;
      });
      empty.hidden = visible.length !== 0;
      wheel.hidden = visible.length === 0;
      center(visible[0]);
    });
  }

  const keypad = document.querySelector("[data-keypad]");
  if (keypad) {
    const pin = document.querySelector("#pin");
    keypad.hidden = false;
    keypad.addEventListener("click", (event) => {
      const key = event.target.closest("[data-pin]");
      if (!key) return;
      const value = key.dataset.pin;
      if (value === "clear") pin.value = "";
      else if (value === "delete") pin.value = pin.value.slice(0, -1);
      else if (pin.value.length < pin.maxLength) pin.value += value;
    });
  }
})();
