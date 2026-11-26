self.addEventListener("push", (event) => {
  event.waitUntil(self.registration.showNotification("Kringkring Push Spike", {
    body: event.data ? event.data.text() : "Kringkring closed-app delivery test",
  }));
});
