// Registers the service worker and keeps this device's push subscription tied to the session.
(() => {
  if (!("serviceWorker" in navigator)) return;
  navigator.serviceWorker.register("/sw.js");
  const prompt = document.querySelector("[data-push]");
  if (!prompt || !("PushManager" in window) || !("Notification" in window)) return;
  const enable = prompt.querySelector("[data-push-enable]");
  const blocked = prompt.querySelector("[data-push-blocked]");

  const save = async (subscription) => {
    const response = await fetch("/push/subscriptions", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(subscription),
    });
    if (!response.ok) throw new Error(`Subscription rejected: ${response.status}`);
  };

  const key = async () => {
    const text = await (await fetch("/push/key")).text();
    const base64 = text.replace(/-/g, "+").replace(/_/g, "/");
    return Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
  };

  const refresh = async () => {
    const registration = await navigator.serviceWorker.ready;
    const subscription = await registration.pushManager.getSubscription();
    const granted = Notification.permission === "granted";
    // Re-saving moves the subscription to the current session after a new sign-in.
    if (granted && subscription) await save(subscription);
    prompt.hidden = granted && Boolean(subscription);
    enable.hidden = Notification.permission === "denied";
    blocked.hidden = Notification.permission !== "denied";
  };

  enable.addEventListener("click", async () => {
    enable.disabled = true;
    try {
      if (await Notification.requestPermission() === "granted") {
        const registration = await navigator.serviceWorker.ready;
        const subscription = await registration.pushManager.getSubscription()
          ?? await registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: await key() });
        await save(subscription);
      }
    } finally {
      enable.disabled = false;
      await refresh();
    }
  });

  refresh().catch(() => {});
})();
