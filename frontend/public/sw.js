const CACHE_NAME = 'payrollmy-shell-v2';
const ASSETS_CACHE = 'payrollmy-assets-v1';
const CACHE_PREFIXES = ['payrollmy-shell-', 'payrollmy-assets-'];
const PRECACHE_URLS = ['/offline.html', '/icons/icon-192.png', '/icons/badge-72.png'];
const DEFAULT_NOTIFICATION_URL = '/portal/notifications';

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE_NAME)
      .then((cache) => cache.addAll(PRECACHE_URLS))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    Promise.all([
      self.registration?.navigationPreload?.enable?.().catch(() => undefined),
      caches
        .keys()
        .then((keys) =>
          Promise.all(
            keys
              .filter(
                (key) =>
                  CACHE_PREFIXES.some((prefix) => key.startsWith(prefix)) &&
                  key !== CACHE_NAME &&
                  key !== ASSETS_CACHE,
              )
              .map((key) => caches.delete(key)),
          ),
        ),
    ]).then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') {
    return;
  }

  let url;
  try {
    url = new URL(request.url);
  } catch {
    return;
  }

  if (url.origin !== self.location.origin || url.pathname.startsWith('/api/')) {
    return;
  }

  if (url.pathname.startsWith('/assets/')) {
    event.respondWith(
      caches.open(ASSETS_CACHE).then(async (cache) => {
        const cached = await cache.match(request);
        if (cached) {
          return cached;
        }
        const response = await fetch(request);
        if (response.ok && response.type === 'basic') {
          void cache.put(request, response.clone());
        }
        return response;
      }),
    );
    return;
  }

  if (request.mode !== 'navigate') {
    return;
  }

  event.respondWith(
    (async () => {
      try {
        const preloaded = await event.preloadResponse;
        if (preloaded) {
          return preloaded;
        }
        return await fetch(request);
      } catch {
        const cached = await caches.match('/offline.html');
        return (
          cached ||
          new Response('Offline', {
            status: 503,
            headers: { 'Content-Type': 'text/plain; charset=utf-8' },
          })
        );
      }
    })(),
  );
});

function sanitizeNotificationUrl(rawUrl) {
  if (typeof rawUrl !== 'string' || !rawUrl) {
    return DEFAULT_NOTIFICATION_URL;
  }
  const trimmed = rawUrl.trim();
  if (trimmed.startsWith('/') && !trimmed.startsWith('//')) {
    return trimmed;
  }
  try {
    const parsed = new URL(trimmed, self.location.origin);
    if (parsed.origin === self.location.origin) {
      return `${parsed.pathname}${parsed.search}${parsed.hash}`;
    }
  } catch {
    // Fall through to default URL.
  }
  return DEFAULT_NOTIFICATION_URL;
}

self.addEventListener('push', (event) => {
  let payload = {};
  if (event.data) {
    try {
      payload = event.data.json();
    } catch {
      payload = { body: event.data.text() };
    }
  }

  const title =
    typeof payload.title === 'string' && payload.title.trim()
      ? payload.title.trim()
      : 'PayrollMY';
  const body = typeof payload.body === 'string' ? payload.body : '';
  const icon =
    typeof payload.icon === 'string' && payload.icon.startsWith('/') && !payload.icon.startsWith('//')
      ? payload.icon
      : '/icons/icon-192.png';
  const badge =
    typeof payload.badge === 'string' &&
    payload.badge.startsWith('/') &&
    !payload.badge.startsWith('//')
      ? payload.badge
      : '/icons/badge-72.png';
  const tag = typeof payload.tag === 'string' && payload.tag ? payload.tag : undefined;
  const targetUrl = sanitizeNotificationUrl(payload.url);

  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      icon,
      badge,
      tag,
      data: { url: targetUrl },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const targetPath = sanitizeNotificationUrl(event.notification?.data?.url);
  const targetUrl = new URL(targetPath, self.location.origin).href;

  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(async (clients) => {
      for (const client of clients) {
        try {
          const clientUrl = new URL(client.url);
          if (clientUrl.origin === self.location.origin) {
            if (
              typeof client.navigate === 'function' &&
              `${clientUrl.pathname}${clientUrl.search}${clientUrl.hash}` !== targetPath
            ) {
              await client.navigate(targetUrl);
            }
            if (typeof client.focus === 'function') {
              return client.focus();
            }
          }
        } catch {
          // Ignore malformed client URLs.
        }
      }
      if (typeof self.clients.openWindow === 'function') {
        return self.clients.openWindow(targetUrl);
      }
      return undefined;
    }),
  );
});

self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting();
  }
});
