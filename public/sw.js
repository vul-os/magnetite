/* Magnetite Service Worker — PWA app shell + offline support */

const CACHE_NAME = 'magnetite-v2';
const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/manifest.json',
  '/favicon.svg',
  '/icons/icon-192.png',
  '/icons/icon-512.png',
];
const API_CACHE_NAME = 'magnetite-api-v2';

/* ── Install: pre-cache app shell ──────────────────────────────────────────── */
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) =>
      cache.addAll(STATIC_ASSETS).catch((err) => {
        // Don't fail install if an icon isn't ready yet — just log it
        console.warn('[SW] Pre-cache warning:', err.message);
      })
    )
  );
  self.skipWaiting();
});

/* ── Activate: purge old caches ────────────────────────────────────────────── */
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((key) => key !== CACHE_NAME && key !== API_CACHE_NAME)
          .map((key) => caches.delete(key))
      )
    )
  );
  self.clients.claim();
});

/* ── Fetch: network-first for API, cache-first for static ─────────────────── */
self.addEventListener('fetch', (event) => {
  const { request } = event;

  // Only intercept same-origin + https (not chrome-extension://, etc.)
  if (!request.url.startsWith('http')) return;

  const url = new URL(request.url);

  // Let browser handle non-GET requests
  if (request.method !== 'GET') return;

  if (url.pathname.startsWith('/api/')) {
    event.respondWith(handleApiRequest(request));
  } else {
    event.respondWith(handleStaticRequest(request));
  }
});

/* Cache-first for static assets, fall back to /index.html for navigation */
async function handleStaticRequest(request) {
  const cached = await caches.match(request);
  if (cached) return cached;

  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(CACHE_NAME);
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    // For navigation requests return cached index.html (SPA offline shell)
    const url = new URL(request.url);
    if (request.mode === 'navigate' || url.pathname === '/') {
      const shell = await caches.match('/index.html');
      if (shell) return shell;
    }
    return new Response('Offline', { status: 503, statusText: 'Service Unavailable' });
  }
}

/* Network-first for API calls, serve stale on failure */
async function handleApiRequest(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(API_CACHE_NAME);
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    const cached = await caches.match(request);
    return (
      cached ||
      new Response(JSON.stringify({ error: 'Offline', offline: true }), {
        status: 503,
        headers: { 'Content-Type': 'application/json' },
      })
    );
  }
}

/* ── Background sync: retry queued mutations ───────────────────────────────── */
self.addEventListener('sync', (event) => {
  if (event.tag === 'sync-data') {
    event.waitUntil(syncPendingData());
  }
});

async function syncPendingData() {
  // No-op for now; mutations use optimistic UI and are re-tried on reconnect
}

/* ── Push notifications ─────────────────────────────────────────────────────── */
self.addEventListener('push', (event) => {
  if (!event.data) return;
  let data = {};
  try { data = event.data.json(); } catch { data = { title: 'Magnetite', body: event.data.text() }; }

  event.waitUntil(
    self.registration.showNotification(data.title || 'Magnetite', {
      body: data.body || '',
      icon: '/icons/icon-192.png',
      badge: '/icons/icon-192.png',
      data: data.url ? { url: data.url } : {},
    })
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const url = event.notification.data?.url || '/';
  event.waitUntil(
    clients.matchAll({ type: 'window' }).then((windowClients) => {
      const existing = windowClients.find((c) => c.url === url && 'focus' in c);
      if (existing) return existing.focus();
      return clients.openWindow(url);
    })
  );
});
