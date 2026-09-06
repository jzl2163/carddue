/* Push-only service worker. Never caches account pages, API replies or credentials. */
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));
self.addEventListener('push', (event) => {
  if (!event.data) return;
  let data;
  try { data = event.data.json(); } catch { return; }
  let link = self.location.origin + '/';
  try {
    const parsed = new URL(data.url || '/', self.location.origin);
    if (parsed.origin === self.location.origin) link = parsed.href;
  } catch { /* Safe fallback. */ }
  event.waitUntil(self.registration.showNotification(String(data.title || 'CardDue'), {
    body: String(data.body || ''), tag: String(data.tag || 'carddue'),
    icon: '/icon.svg', data: { url: link }
  }));
});
self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const link = new URL(event.notification.data?.url || '/', self.location.origin);
  if (link.origin !== self.location.origin) return;
  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    for (const window of windows) {
      if (new URL(window.url).origin === self.location.origin) {
        await window.navigate(link.href); return window.focus();
      }
    }
    return self.clients.openWindow(link.href);
  })());
});
