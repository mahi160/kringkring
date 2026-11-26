// Shows ক্রিং ক্রিং pushes and routes notification taps back into the app.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("push", (event) => {
  const data = event.data ? event.data.json() : {};
  const options = {
    body: data.body,
    tag: data.tag,
    renotify: true,
    icon: "/icon-192.png",
    data: { url: data.url, request: data.actions?.request },
  };
  // Android shows these buttons; iOS ignores them and a tap opens the card.
  if (data.actions) {
    options.actions = [
      { action: "accept", title: data.actions.accept },
      { action: "reject", title: data.actions.reject },
    ];
  }
  event.waitUntil(self.registration.showNotification(data.title || "ক্রিং ক্রিং", options));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const { url = "/", request } = event.notification.data || {};
  event.waitUntil((async () => {
    let target = url;
    if (request && event.action === "accept") {
      await fetch(`/staff/requests/${request}`, {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded" },
        body: "action=accept",
        redirect: "manual",
      }).catch(() => {});
    } else if (request && event.action === "reject") {
      target = `/staff/requests/${request}/reject`;
    }
    const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    for (const client of windows) {
      try {
        await client.focus();
        await client.navigate(target);
        return;
      } catch {
        // Uncontrolled or cross-origin windows can't navigate; open a new one instead.
      }
    }
    await self.clients.openWindow(target);
  })());
});
