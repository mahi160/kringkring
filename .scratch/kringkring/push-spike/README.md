# Kringkring real-phone push spike

Use a private test Wi‑Fi network and test phones. Both phones and the host need internet access for Apple/Google push. Do not expose these ports publicly: anyone on the test LAN can open the spike and trigger notifications. Stop the spike and remove its CA from all devices when done.

## Start

From this directory, use the host's actual Wi‑Fi IPv4 address and a real contact email (VAPID subject):

```sh
cargo run --locked -- <LAN-IP> <contact-email>
```

Allow TCP 8080 and 8443 from the test LAN. Keep the process running through both tests: it generates a fresh CA and VAPID key and keeps subscriptions in memory. After a restart, remove the old CA, uninstall the spike and clear its site data before repeating.

The host serves `http://<LAN-IP>:8080/ca.cer` and `https://<LAN-IP>:8443/`. The CA download is plaintext: use a trusted copy from the host (for example, download locally and transfer directly), or verify the downloaded file against the host copy out of band before trusting it. Never bypass the HTTPS certificate warning.

## iPhone (iOS 16.4+)

1. Open the CA file. After accepting the profile download, install it via **Settings → Profile Downloaded** or **General → VPN & Device Management**. Turn on full trust for **Kringkring Spike CA** under **Settings → General → About → Certificate Trust Settings** ([Apple](https://support.apple.com/en-us/102390)).
2. Load the HTTPS URL in Safari without a certificate warning. Tap **Share → Add to Home Screen**, enable **Open as Web App** if offered, then add it.
3. Open it from the Home Screen. Tap **Subscribe**, allow notifications, and wait for **Subscribed** ([WebKit](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/)).
4. Return to the Home Screen and dismiss the spike from the app switcher. Send from a different device or the host, as below. Record whether a notification appeared without reopening the app. If it fails, stop the project rollout and revisit TLS before moving on.

## Android (Chrome)

1. Save the CA file. On Pixel, open **Settings → Security & privacy → More security settings → Encryption & credentials → Install a certificate → CA certificate** and select the file. Other Android vendors may use different labels ([Google](https://support.google.com/pixelphone/answer/2844832)).
2. Load the HTTPS URL in Chrome without a certificate warning. Open the Chrome menu and choose **Add to Home screen → Install** (labels vary by version).
3. Open from the Home Screen. Tap **Subscribe**, grant notifications, and wait for **Subscribed**. Check that Chrome notifications are allowed in Android settings if prompted.
4. Return to the Home Screen and dismiss the spike from Recents. Do not use Android's **Force stop** against Chrome; that is a different test. Send from a different device or the host and record delivery separately.

## Send and record

Use **Send test push** on another device that trusts the CA, or run this from the host (replace both placeholders):

```sh
curl -fsS 'http://<LAN-IP>:8080/ca.cer' -o kringkring-spike-ca.cer
openssl x509 -inform DER -in kringkring-spike-ca.cer -out kringkring-spike-ca.pem
curl -fsS --cacert kringkring-spike-ca.pem \
  -H 'Origin: https://<LAN-IP>:8443' -X POST 'https://<LAN-IP>:8443/send'
```

Test iOS first, before subscribing Android, so “accepted: 1” identifies the iPhone's push service. After Android subscribes, the server sends to both. Acceptance only proves the push service accepted the request, not that a phone showed it. Check each phone without reopening the app. Record OS/browser versions, actual install steps, permission and TLS outcomes, delivery, and gotchas (e.g. Focus, battery saver or Wi‑Fi isolation) in ticket 01 and SPEC.md §10. Keep the spike separate from the production app; archive or remove it after recording results.
