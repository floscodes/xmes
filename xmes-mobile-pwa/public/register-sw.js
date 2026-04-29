// ── Service Worker registration + Push subscription ──────────────────────────
//
// NOTE: window.XMES_PUSH_WORKER_URL is set by WASM *after* this script runs,
// so we must read it lazily (at call time) inside each function — never capture
// it as a top-level constant.

// Register SW (no user gesture needed)
(async function () {
  if (!('serviceWorker' in navigator)) return;
  try {
    await navigator.serviceWorker.register('/sw.js');
  } catch (e) {
    console.warn('[xmes] SW registration failed:', e);
  }
})();

// ── Called from Rust on mount when permission is still 'default' ─────────────
// Registers a one-shot capture-phase listener so the NEXT natural user tap
// triggers the permission dialog (guaranteed user-gesture context on iOS).
window.xmesEnablePushOnNextTap = function () {
  if (!window.XMES_PUSH_WORKER_URL) return;
  if (typeof Notification === 'undefined' || Notification.permission !== 'default') return;
  const handler = async function () {
    document.removeEventListener('touchend', handler, true);
    document.removeEventListener('click',    handler, true);
    await window.xmesRequestPushPermission();
  };
  document.addEventListener('touchend', handler, { capture: true, once: true });
  document.addEventListener('click',    handler, { capture: true, once: true });
};

// ── Called from Rust after XMES_INBOX_ID is set ──────────────────────────────
// Auto-subscribes silently when permission is already granted.
window.xmesSubscribePush = async function () {
  const pushUrl = window.XMES_PUSH_WORKER_URL;
  if (!pushUrl) return;
  if (!('PushManager' in window)) return;
  if (typeof Notification === 'undefined') return;
  if (Notification.permission !== 'granted') return;

  const inboxId = window.XMES_INBOX_ID;
  if (!inboxId) return;

  try {
    const sw  = await navigator.serviceWorker.ready;
    let sub   = await sw.pushManager.getSubscription();

    const res           = await fetch(`${pushUrl}/vapid-public-key`);
    const { publicKey } = await res.json();
    if (!publicKey) { console.warn('[xmes] no VAPID public key'); return; }

    // If an existing subscription uses a different server key, unsubscribe first
    // so we always register a fresh FCM/APNs subscription.
    if (sub) {
      const existingKey = sub.options && sub.options.applicationServerKey
        ? btoa(String.fromCharCode(...new Uint8Array(sub.options.applicationServerKey)))
            .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '')
        : null;
      if (existingKey !== publicKey) {
        console.log('[xmes] server key mismatch, resubscribing');
        await sub.unsubscribe();
        sub = null;
      }
    }

    if (!sub) {
      console.log('[xmes] creating new push subscription');
      sub = await sw.pushManager.subscribe({
        userVisibleOnly:      true,
        applicationServerKey: urlBase64ToUint8Array(publicKey),
      });
      console.log('[xmes] subscribed:', sub.endpoint);
    }

    const body = { inbox_id: inboxId, subscription: sub.toJSON() };
    if (window.XMES_ETH_ADDRESS) body.address = window.XMES_ETH_ADDRESS;
    const r = await fetch(`${pushUrl}/subscribe`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify(body),
    });
    console.log('[xmes] subscription registered, status:', r.status);
  } catch (e) {
    console.warn('[xmes] xmesSubscribePush failed:', e);
  }
};

// ── Called from a user-gesture context (touchend handler) ────────────────────
// Requests permission, then subscribes.
window.xmesRequestPushPermission = async function () {
  const pushUrl = window.XMES_PUSH_WORKER_URL;
  if (!pushUrl) return;
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
