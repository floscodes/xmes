// ── Service Worker registration + Push subscription ──────────────────────────
//
// PUSH_WORKER_URL and VAPID_PUBLIC_KEY are injected at build time or set here.
// Override by defining window.XMES_PUSH_WORKER_URL before this script loads.

const PUSH_WORKER_URL = window.XMES_PUSH_WORKER_URL ?? '';

if ('serviceWorker' in navigator) {
  window.addEventListener('load', async function () {
    let reg;
    try {
      reg = await navigator.serviceWorker.register('/sw.js');
      console.log('[xmes] SW registered, scope:', reg.scope);
    } catch (err) {
      console.warn('[xmes] SW registration failed:', err);
      return;
    }

    if (!PUSH_WORKER_URL) return; // push server not configured

    // Subscribe to push once we have a VAPID public key from the server
    try {
      const res  = await fetch(`${PUSH_WORKER_URL}/vapid-public-key`);
      const json = await res.json();
      const vapidPublicKey = json.publicKey;
      if (!vapidPublicKey) return;

      // Wait for SW to be active
      await navigator.serviceWorker.ready;
      const sw = await navigator.serviceWorker.ready;

      // Check existing subscription first
      let sub = await sw.pushManager.getSubscription();
      if (!sub) {
        const permission = await Notification.requestPermission();
        if (permission !== 'granted') return;

        sub = await sw.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: urlBase64ToUint8Array(vapidPublicKey),
        });
      }

      // Register subscription with push server; called on every load so the
      // server always has a fresh endpoint (endpoints can change).
      const inboxId = window.XMES_INBOX_ID; // set by the Rust app via js_sys::eval
      if (!inboxId) return;

      await fetch(`${PUSH_WORKER_URL}/subscribe`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ inbox_id: inboxId, subscription: sub.toJSON() }),
      });
    } catch (err) {
      console.warn('[xmes] Push subscription failed:', err);
    }
  });
}

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - base64String.length % 4) % 4);
  const base64  = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
  const raw     = atob(base64);
  return Uint8Array.from([...raw].map(c => c.charCodeAt(0)));
}
