const subscribe = document.querySelector("#subscribe");
const send = document.querySelector("#send");
const status = document.querySelector("#status");

async function request(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) throw new Error(await response.text());
  return response;
}

async function prepareSubscription() {
  if (!("serviceWorker" in navigator) || !("PushManager" in window)) {
    status.textContent = "Push is unavailable here. On iOS, open the installed Home Screen app.";
    return;
  }

  await navigator.serviceWorker.register("/sw.js");
  const registration = await navigator.serviceWorker.ready;
  const applicationServerKey = await (await request("/vapid-public-key")).json();
  status.textContent = "Ready to subscribe.";
  subscribe.disabled = false;
  subscribe.addEventListener("click", async () => {
    subscribe.disabled = true;
    try {
      // Call subscribe directly from the tap, before any other asynchronous work.
      const subscription = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey,
      });
      await request("/subscriptions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(subscription),
      });
      status.textContent = "Subscribed. Close the app and send from another device.";
    } catch (error) {
      status.textContent = `Subscribe failed: ${error.message}`;
    } finally {
      subscribe.disabled = false;
    }
  });
}

send.addEventListener("click", async () => {
  send.disabled = true;
  status.textContent = "Sending…";
  try {
    status.textContent = await (await request("/send", { method: "POST" })).text();
  } catch (error) {
    status.textContent = `Send failed: ${error.message}`;
  } finally {
    send.disabled = false;
  }
});

prepareSubscription().catch((error) => {
  status.textContent = `Setup failed: ${error.message}`;
});
