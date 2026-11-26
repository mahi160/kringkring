const feedback = document.querySelector("[data-tile-feedback]");
for (const tile of document.querySelectorAll("[data-preview-tile]")) {
  tile.addEventListener("click", () => {
    feedback.textContent = feedback.dataset.prefix + tile.textContent.trim() + feedback.dataset.suffix;
  });
}
