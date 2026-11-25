import type { PushSubscriptionPayload } from '@/api/notifications';

export interface BeforeInstallPromptEvent extends Event {
  readonly platforms: string[];
  readonly userChoice: Promise<{ outcome: 'accepted' | 'dismissed'; platform: string }>;
  prompt(): Promise<void>;
}

export type PwaPlatform = 'ios-safari' | 'ios-other' | 'android' | 'desktop';

export type NotificationStatus =
  | 'default'
  | 'granted'
  | 'denied'
  | 'ios-install-required'
  | 'insecure-context'
  | 'unsupported';

const INSTALL_DISMISSED_KEY = 'payroll-pwa-install-dismissed-at';
const INSTALL_DISMISS_TTL_MS = 14 * 24 * 60 * 60 * 1000;

let deferredInstallPrompt: BeforeInstallPromptEvent | null = null;
let listenersAttached = false;
let swRegistrationPromise: Promise<ServiceWorkerRegistration | null> | null = null;
const promptSubscribers = new Set<() => void>();

function notifyPromptSubscribers(): void {
  promptSubscribers.forEach((listener) => listener());
}

function handleBeforeInstallPrompt(event: Event): void {
  event.preventDefault();
  deferredInstallPrompt = event as BeforeInstallPromptEvent;
  notifyPromptSubscribers();
}

function handleAppInstalled(): void {
  deferredInstallPrompt = null;
  notifyPromptSubscribers();
}

/**
 * Attaches early window listeners for `beforeinstallprompt` and `appinstalled`.
 * Safe to invoke multiple times (idempotent).
 */
export function initPwaListeners(): void {
  if (typeof window === 'undefined' || listenersAttached) {
    return;
  }
  listenersAttached = true;
  window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
  window.addEventListener('appinstalled', handleAppInstalled);
}

export function getDeferredInstallPrompt(): BeforeInstallPromptEvent | null {
  return deferredInstallPrompt;
}

export function clearDeferredInstallPrompt(): void {
  if (deferredInstallPrompt === null) {
    return;
  }
  deferredInstallPrompt = null;
  notifyPromptSubscribers();
}

export function subscribeInstallPrompt(listener: () => void): () => void {
  promptSubscribers.add(listener);
  return () => {
    promptSubscribers.delete(listener);
  };
}

export function resetPwaStoreForTests(): void {
  deferredInstallPrompt = null;
  swRegistrationPromise = null;
  promptSubscribers.clear();
  if (typeof window !== 'undefined' && listenersAttached) {
    window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    window.removeEventListener('appinstalled', handleAppInstalled);
  }
  listenersAttached = false;
}

export function detectPwaPlatform(
  ua?: string,
  navPlatform?: string,
  maxTouchPoints?: number,
): PwaPlatform {
  const resolvedUa =
    ua ?? (typeof navigator !== 'undefined' ? navigator.userAgent : '');
  const resolvedPlatform =
    navPlatform ?? (typeof navigator !== 'undefined' ? navigator.platform : '');
  const resolvedTouchPoints =
    maxTouchPoints ??
    (typeof navigator !== 'undefined' ? navigator.maxTouchPoints ?? 0 : 0);

  const isIosDevice =
    /iPhone|iPad|iPod/i.test(resolvedUa) ||
    (resolvedPlatform === 'MacIntel' && resolvedTouchPoints > 1);

  if (isIosDevice) {
    const isThirdPartyOrInApp =
      /CriOS|FxiOS|EdgiOS|OPiOS|DuckDuckGo|FBAN|FBAV|Instagram|Line|MicroMessenger/i.test(
        resolvedUa,
      ) || !/Safari/i.test(resolvedUa);
    return isThirdPartyOrInApp ? 'ios-other' : 'ios-safari';
  }

  if (/Android/i.test(resolvedUa)) {
    return 'android';
  }

  return 'desktop';
}

export function isIosPlatform(platform: PwaPlatform): boolean {
  return platform === 'ios-safari' || platform === 'ios-other';
}

export function isRunningStandalone(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  const matchesStandaloneMedia =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(display-mode: standalone)').matches;
  const navigatorStandalone =
    (window.navigator as Navigator & { standalone?: boolean }).standalone === true;
  return matchesStandaloneMedia || navigatorStandalone;
}

/**
 * Resolves the effective notification permission state.
 * Checks iOS Home Screen installation BEFORE `window.Notification`, because iOS
 * Safari keeps `window.Notification` undefined until opened as an installed app.
 */
export function getNotificationStatus(
  platform: PwaPlatform,
  standalone: boolean,
): NotificationStatus {
  if (isIosPlatform(platform) && !standalone) {
    return 'ios-install-required';
  }
  if (typeof window === 'undefined') {
    return 'unsupported';
  }
  if (window.isSecureContext === false) {
    return 'insecure-context';
  }
  if (typeof window.Notification === 'undefined') {
    return 'unsupported';
  }
  return window.Notification.permission;
}

export function isPushSupported(): boolean {
  if (typeof window === 'undefined' || window.isSecureContext === false) {
    return false;
  }
  return (
    'serviceWorker' in navigator &&
    'PushManager' in window &&
    typeof window.Notification !== 'undefined'
  );
}

export function registerServiceWorker(): Promise<ServiceWorkerRegistration | null> {
  if (
    typeof window === 'undefined' ||
    !('serviceWorker' in navigator) ||
    window.isSecureContext === false
  ) {
    return Promise.resolve(null);
  }
  if (!swRegistrationPromise) {
    swRegistrationPromise = navigator.serviceWorker
      .register('/sw.js', {
        scope: '/',
        updateViaCache: 'none',
      })
      .catch(() => {
        swRegistrationPromise = null;
        return null;
      });
  }
  return swRegistrationPromise;
}

export function registerServiceWorkerOnLoad(
  onReady?: (registration: ServiceWorkerRegistration | null) => void,
): () => void {
  if (typeof window === 'undefined') {
    return () => {};
  }
  const run = () => {
    void registerServiceWorker().then((registration) => onReady?.(registration));
  };
  if (document.readyState === 'complete') {
    run();
    return () => {};
  }
  window.addEventListener('load', run, { once: true });
  return () => window.removeEventListener('load', run);
}

/**
 * Awaits an active ServiceWorkerRegistration (bounded by `timeoutMs`) so
 * `registration.showNotification()` and `pushManager.subscribe()` do not fail
 * during the first-install `installing` -> `activated` transition.
 */
export async function waitForActiveServiceWorker(
  timeoutMs = 3000,
): Promise<ServiceWorkerRegistration | null> {
  if (
    typeof window === 'undefined' ||
    !('serviceWorker' in navigator) ||
    window.isSecureContext === false
  ) {
    return null;
  }

  const registered = await registerServiceWorker();
  if (registered && (!('active' in registered) || registered.active)) {
    return registered;
  }

  if ('ready' in navigator.serviceWorker && navigator.serviceWorker.ready) {
    let timerId: ReturnType<typeof setTimeout> | undefined;
    try {
      const readyReg = await Promise.race<ServiceWorkerRegistration | null>([
        navigator.serviceWorker.ready,
        new Promise<null>((resolve) => {
          timerId = setTimeout(() => resolve(null), timeoutMs);
        }),
      ]);
      if (readyReg) {
        return readyReg;
      }
    } catch {
      // Fall back to getRegistration below.
    } finally {
      if (timerId !== undefined) {
        clearTimeout(timerId);
      }
    }
  }

  if (typeof navigator.serviceWorker.getRegistration === 'function') {
    try {
      return (await navigator.serviceWorker.getRegistration('/')) ?? registered;
    } catch {
      return registered;
    }
  }

  return registered;
}

/**
 * Calls `Notification.requestPermission()` immediately without preceding async
 * work so iOS Safari preserves the required user-gesture activation context.
 */
export function requestNotificationPermission(): Promise<
  NotificationPermission | 'unsupported'
> {
  if (typeof window === 'undefined' || typeof window.Notification === 'undefined') {
    return Promise.resolve('unsupported');
  }
  return window.Notification.requestPermission();
}

export async function showLocalNotification(
  title: string,
  options?: { body?: string; tag?: string; url?: string },
): Promise<boolean> {
  if (
    typeof window === 'undefined' ||
    typeof window.Notification === 'undefined' ||
    window.Notification.permission !== 'granted'
  ) {
    return false;
  }

  const notificationOptions: NotificationOptions = {
    body: options?.body,
    tag: options?.tag,
    icon: '/icons/icon-192.png',
    badge: '/icons/badge-72.png',
    data: { url: options?.url ?? '/portal/notifications' },
  };

  try {
    const registration = await waitForActiveServiceWorker();
    if (
      registration &&
      typeof registration.showNotification === 'function' &&
      (!('active' in registration) || registration.active)
    ) {
      await registration.showNotification(title, notificationOptions);
      return true;
    }

    new window.Notification(title, notificationOptions);
    return true;
  } catch {
    return false;
  }
}

export function urlBase64ToUint8Array(
  base64String: string,
): Uint8Array<ArrayBuffer> {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
  const rawData = window.atob(base64);
  const buffer = new ArrayBuffer(rawData.length);
  const outputArray = new Uint8Array(buffer);
  for (let i = 0; i < rawData.length; i += 1) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.byteLength !== b.byteLength) {
    return false;
  }
  for (let i = 0; i < a.byteLength; i += 1) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}

export function serializePushSubscription(
  sub: PushSubscription,
): PushSubscriptionPayload | null {
  const json = typeof sub.toJSON === 'function' ? sub.toJSON() : null;
  const endpoint = json?.endpoint ?? sub.endpoint;
  const p256dh = json?.keys?.p256dh;
  const auth = json?.keys?.auth;
  if (!endpoint || !p256dh || !auth) {
    return null;
  }
  return {
    endpoint,
    keys: { p256dh, auth },
  };
}

export async function subscribeToPush(
  registration: ServiceWorkerRegistration,
  vapidPublicKey: string,
): Promise<PushSubscriptionPayload | null> {
  const trimmedKey = vapidPublicKey.trim();
  if (!trimmedKey || !('pushManager' in registration)) {
    return null;
  }

  try {
    const targetKeyBytes = urlBase64ToUint8Array(trimmedKey);
    const existing = await registration.pushManager.getSubscription();
    if (existing) {
      const existingKey = existing.options?.applicationServerKey;
      if (existingKey) {
        const existingBytes = new Uint8Array(existingKey);
        if (bytesEqual(existingBytes, targetKeyBytes)) {
          return serializePushSubscription(existing);
        }
        await existing.unsubscribe();
      } else {
        return serializePushSubscription(existing);
      }
    }

    const created = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: targetKeyBytes,
    });
    return serializePushSubscription(created);
  } catch {
    return null;
  }
}

export async function getCurrentPushEndpoint(): Promise<string | null> {
  if (
    typeof window === 'undefined' ||
    !('serviceWorker' in navigator) ||
    typeof navigator.serviceWorker.getRegistration !== 'function'
  ) {
    return null;
  }
  try {
    const registration = await navigator.serviceWorker.getRegistration('/');
    if (!registration || !('pushManager' in registration)) {
      return null;
    }
    const existing = await registration.pushManager.getSubscription();
    return existing?.endpoint ?? null;
  } catch {
    return null;
  }
}

export async function unsubscribeFromPush(): Promise<string | null> {
  if (
    typeof window === 'undefined' ||
    !('serviceWorker' in navigator) ||
    typeof navigator.serviceWorker.getRegistration !== 'function'
  ) {
    return null;
  }
  try {
    const registration = await navigator.serviceWorker.getRegistration('/');
    if (!registration || !('pushManager' in registration)) {
      return null;
    }
    const existing = await registration.pushManager.getSubscription();
    if (!existing) {
      return null;
    }
    const endpoint = existing.endpoint;
    await existing.unsubscribe();
    return endpoint;
  } catch {
    return null;
  }
}

export function updateAppBadge(unreadCount: number): void {
  if (typeof navigator === 'undefined') {
    return;
  }
  const badgeNav = navigator as Navigator & {
    setAppBadge?: (contents?: number) => Promise<void>;
    clearAppBadge?: () => Promise<void>;
  };
  try {
    if (unreadCount > 0 && typeof badgeNav.setAppBadge === 'function') {
      void badgeNav.setAppBadge(unreadCount).catch(() => undefined);
    } else if (unreadCount <= 0 && typeof badgeNav.clearAppBadge === 'function') {
      void badgeNav.clearAppBadge().catch(() => undefined);
    }
  } catch {
    // Ignore unsupported badging environments.
  }
}

export function isInstallBannerDismissed(): boolean {
  try {
    const raw = localStorage.getItem(INSTALL_DISMISSED_KEY);
    if (!raw) {
      return false;
    }
    const timestamp = Number(raw);
    if (!Number.isFinite(timestamp)) {
      return false;
    }
    return Date.now() - timestamp < INSTALL_DISMISS_TTL_MS;
  } catch {
    return false;
  }
}

export function dismissInstallBanner(): void {
  try {
    localStorage.setItem(INSTALL_DISMISSED_KEY, String(Date.now()));
  } catch {
    // Ignore storage access errors in restricted/test environments.
  }
}
