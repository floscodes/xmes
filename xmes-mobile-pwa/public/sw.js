const CACHE = 'xmes-v3';

const PRECACHE = [
  '/',
  '/assets/manifest.webmanifest',
  '/assets/icons/icon-192x192.png',
  '/assets/icons/icon-512x512.png',
];

// Install: precache shell assets
self.addEventListener('install', event => {
  self.skipWaiting();
  event.waitUntil(
    caches.open(CACHE).then(cache => cache.addAll(PRECACHE))
  );
});

// Activate: remove old caches
self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

// ── Badge count (persisted via Cache Storage) ───────────────────────────────

async function getBadgeCount() {
  try {
    const cache = await caches.open('xmes-badge');
    const res   = await cache.match('count');
    return res ? (parseInt(await res.text(), 10) || 0) : 0;
  } catch (_) { return 0; }
}

async function setBadgeCount(n) {
  try {
    const cache = await caches.open('xmes-badge');
    if (n > 0) {
      await cache.put('count', new Response(String(n)));
      if ('setAppBadge' in self.registration) await self.registration.setAppBadge(n);
      else if ('setAppBadge' in navigator) await navigator.setAppBadge(n);
    } else {
      await cache.delete('count');
      if ('clearAppBadge' in self.registration) await self.registration.clearAppBadge();
      else if ('clearAppBadge' in navigator) await navigator.clearAppBadge();
    }
  } catch (_) {}
}

// ── Push notifications ──────────────────────────────────────────────────────

self.addEventListener('push', event => {
  let title = 'xmes';
  let body  = 'New message';
  let data  = {};

  if (event.data) {
    try {
      data  = event.data.json();
      title = data.title ?? title;
      body  = data.body  ?? body;
    } catch (_) {}
  }

  event.waitUntil(
    getBadgeCount().then(count => {
      const next = count + 1;
      return setBadgeCount(next).then(() =>
        self.registration.showNotification(title, {
          body,
          icon:     '/assets/icons/icon-192x192.png',
          badge:    '/assets/icons/icon-96x96.png',
          tag:      'xmes-message',
          renotify: true,
          data:     { ...data, badgeCount: next },
        })
      );
    })
  );
});

self.addEventListener('notificationclick', event => {
  event.notification.close();
  event.waitUntil(
    setBadgeCount(0).then(() =>
      clients.matchAll({ type: 'window', includeUncontrolled: true }).then(list => {
        for (const client of list) {
          if ('focus' in client) return client.focus();
        }
        if (clients.openWindow) return clients.openWindow('/');
      })
    )
  );
});

// ── Message from app: clear badge when app comes to foreground ───────────────

self.addEventListener('message', event => {
  if (event.data?.type === 'clear-badge') setBadgeCount(0);
  if (event.data?.type === 'sync-badge')  setBadgeCount(event.data.count ?? 0);
});

// Fetch: network-first for navigation, cache-first for assets
self.addEventListener('fetch', event => {
  const { request } = event;
  const url = new URL(request.url);

  // Only handle same-origin requests
  if (url.origin !== self.location.origin) return;

  // Navigation requests: network-first, fall back to cached '/'
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).catch(() => caches.match('/'))
    );
    return;
  }

  // Static assets (.wasm, .js, .css, images): cache-first
  if (/\.(wasm|js|css|png|svg|ico|webmanifest)$/.test(url.pathname)) {
    event.respondWith(
      caches.match(request).then(cached => {
        if (cached) return cached;
        return fetch(request).then(response => {
          if (response.ok) {
            const clone = response.clone();
            caches.open(CACHE).then(cache => cache.put(request, clone));
          }
          return response;
        });
      })
    );
  }
});
