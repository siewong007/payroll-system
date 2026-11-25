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
    if (
      'serviceWorker' in navigator &&
      typeof navigator.serviceWorker.getRegistration === 'function'
    ) {
      try {
        const registration = await navigator.serviceWorker.getRegistration('/');
        if (
          registration &&
          typeof registration.showNotification === 'function' &&
          (!('active' in registration) || registration.active)
        ) {
          await registration.showNotification(title, notificationOptions);
          return true;
        }
      } catch {
        // Fall through to window.Notification if SW is still activating or fails.
      }
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

export async function subscribeToPushIfConfigured(
  registration: ServiceWorkerRegistration,
): Promise<PushSubscription | null> {
  const vapidPublicKey = import.meta.env.VITE_VAPID_PUBLIC_KEY;
  if (
    typeof vapidPublicKey !== 'string' ||
    !vapidPublicKey.trim() ||
    !('pushManager' in registration)
  ) {
    return null;
  }

  try {
    const existing = await registration.pushManager.getSubscription();
    if (existing) {
      return existing;
    }
    return await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(vapidPublicKey.trim()),
    });
  } catch {
    return null;
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
