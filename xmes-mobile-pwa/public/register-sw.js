// ── Service Worker registration + Push subscription ──────────────────────────

const PUSH_WORKER_URL = window.XMES_PUSH_WORKER_URL ?? '';

// Register SW (no user gesture needed)
(async function () {
  if (!('serviceWorker' in navigator)) return;
  try {
    await navigator.serviceWorker.register('/sw.js');
  } catch (e) {
    console.warn('[xmes] SW registration failed:', e);
  }
})();

// ── Called from Rust after XMES_INBOX_ID is set ──────────────────────────────
// Auto-subscribes silently when permission is already granted.
window.xmesSubscribePush = async function () {
  if (!PUSH_WORKER_URL) return;
  if (!('PushManager' in window)) return;
  if (typeof Notification === 'undefined') return;
  if (Notification.permission !== 'granted') return;

  const inboxId = window.XMES_INBOX_ID;
  if (!inboxId) return;

  try {
    const sw  = await navigator.serviceWorker.ready;
    let sub   = await sw.pushManager.getSubscription();

    if (!sub) {
      const res        = await fetch(`${PUSH_WORKER_URL}/vapid-public-key`);
      const { publicKey } = await res.json();
      if (!publicKey) return;
      sub = await sw.pushManager.subscribe({
        userVisibleOnly:      true,
        applicationServerKey: urlBase64ToUint8Array(publicKey),
      });
    }

    await fetch(`${PUSH_WORKER_URL}/subscribe`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify({ inbox_id: inboxId, subscription: sub.toJSON() }),
    });
  } catch (e) {
    console.warn('[xmes] xmesSubscribePush failed:', e);
  }
};

// ── Called from a Rust onclick handler (user gesture) ────────────────────────
// Requests permission, then subscribes. Must be triggered by a user tap.
window.xmesRequestPushPermission = async function () {
  if (!PUSH_WORKER_URL) return;
  if (!('Notification' in window) || !('PushManager' in window)) return;

  try {
    const perm = await Notification.requestPermission();
    if (perm !== 'granted') return;
    await window.xmesSubscribePush();
  } catch (e) {
    console.warn('[xmes] xmesRequestPushPermission failed:', e);
  }
};

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
  const base64  = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
  const raw     = atob(base64);
  return Uint8Array.from([...raw].map(c => c.charCodeAt(0)));
}
