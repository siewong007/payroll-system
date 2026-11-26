const CACHE_NAME = 'payrollmy-shell-v3';
const ASSETS_CACHE = 'payrollmy-assets-v2';
const MAX_CACHED_ASSETS = 250;
const CACHE_PREFIXES = ['payrollmy-shell-', 'payrollmy-assets-'];
const PRECACHE_URLS = ['/offline.html', '/icons/icon-192.png', '/icons/badge-72.png'];
const DEFAULT_NOTIFICATION_URL = '/portal/notifications';

async function pruneAssetsCache(cache) {
  const keys = await cache.keys();
  const excess = keys.length - MAX_CACHED_ASSETS;
  if (excess > 0) {
    await Promise.all(keys.slice(0, excess).map((req) => cache.delete(req)));
  }
}

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

self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting();
  }
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
        const contentType = (response.headers.get('content-type') || '').toLowerCase();
        if (response.ok && response.type === 'basic' && !contentType.includes('text/html')) {
          await cache.put(request, response.clone());
          void pruneAssetsCache(cache);
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

function hasControlOrBackslash(value) {
  for (let i = 0; i < value.length; i += 1) {
    const code = value.charCodeAt(i);
    if (code <= 0x1f || code === 0x7f || code === 0x5c) {
      return true;
    }
  }
  return false;
}

function isSafeRelativeAssetPath(value) {
  return (
    typeof value === 'string' &&
    value.startsWith('/icons/') &&
    !value.includes('..') &&
    !hasControlOrBackslash(value)
  );
}

function sanitizeNotificationUrl(rawUrl) {
  if (typeof rawUrl !== 'string' || !rawUrl) {
    return DEFAULT_NOTIFICATION_URL;
  }
  const trimmed = rawUrl.trim();
  if (!trimmed || trimmed.startsWith('//') || hasControlOrBackslash(trimmed)) {
    return DEFAULT_NOTIFICATION_URL;
  }
  try {
    const parsed = new URL(trimmed, self.location.origin);
    if (
      (parsed.protocol === 'http:' || parsed.protocol === 'https:') &&
      parsed.origin === self.location.origin &&
      parsed.pathname.startsWith('/') &&
      !parsed.pathname.startsWith('//') &&
      !hasControlOrBackslash(parsed.pathname)
    ) {
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
  const icon = isSafeRelativeAssetPath(payload.icon) ? payload.icon : '/icons/icon-192.png';
  const badge = isSafeRelativeAssetPath(payload.badge) ? payload.badge : '/icons/badge-72.png';
  const tag = typeof payload.tag === 'string' && payload.tag ? payload.tag : undefined;
  const targetUrl = sanitizeNotificationUrl(payload.url);

  const tasks = [
    self.registration.showNotification(title, {
      body,
      icon,
      badge,
      tag,
      renotify: Boolean(tag),
      data: { url: targetUrl },
    }),
  ];

  if (typeof payload.unread_count === 'number' && self.navigator) {
    if (payload.unread_count > 0 && typeof self.navigator.setAppBadge === 'function') {
      tasks.push(self.navigator.setAppBadge(payload.unread_count).catch(() => undefined));
    } else if (
      payload.unread_count <= 0 &&
      typeof self.navigator.clearAppBadge === 'function'
    ) {
      tasks.push(self.navigator.clearAppBadge().catch(() => undefined));
    }
  }

  event.waitUntil(Promise.all(tasks));
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const targetPath = sanitizeNotificationUrl(event.notification?.data?.url);
  let targetUrl;
  try {
    const resolved = new URL(targetPath, self.location.origin);
    if (resolved.origin !== self.location.origin) {
      return;
    }
    targetUrl = resolved.href;
  } catch {
    return;
  }

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

self.addEventListener('pushsubscriptionchange', (event) => {
  event.waitUntil(
    (async () => {
      try {
        const key = event.oldSubscription?.options?.applicationServerKey;
        if (key && self.registration?.pushManager?.subscribe) {
          await self.registration.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: key,
          });
        }
      } catch {
        // Ignore if re-subscribe requires foreground context.
      }
      const clients = await self.clients.matchAll({
        type: 'window',
        includeUncontrolled: true,
      });
      for (const client of clients) {
        client.postMessage({ type: 'PUSH_SUBSCRIPTION_CHANGED' });
      }
    })(),
  );
});
