import fs from 'node:fs';
import path from 'node:path';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '@/components/ui/Toast';
import { PwaProvider } from '@/context/PwaProvider';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';
import { PwaInstallBanner } from '@/components/pwa/PwaInstallBanner';
import {
  detectPwaPlatform,
  getNotificationStatus,
  resetPwaStoreForTests,
  subscribeToPush,
  updateAppBadge,
  urlBase64ToUint8Array,
  type BeforeInstallPromptEvent,
} from '@/lib/pwa';
import type { CompanySetting } from '@/types';

const settingsMocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  bulkUpdateSettings: vi.fn(),
}));

const authMocks = vi.hoisted(() => ({
  useAuth: vi.fn(),
}));

const notificationApiMocks = vi.hoisted(() => ({
  getNotificationCount: vi.fn(),
  getNotifications: vi.fn(),
  getPushConfig: vi.fn(),
  savePushSubscription: vi.fn(),
  deletePushSubscription: vi.fn(),
  sendTestPush: vi.fn(),
}));

vi.mock('@/api/settings', () => settingsMocks);
vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/notifications', () => notificationApiMocks);

import { SettingsPage } from '@/pages/settings/SettingsPage';

const originalUserAgentDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'userAgent',
);
const originalPlatformDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'platform',
);
const originalMaxTouchPointsDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'maxTouchPoints',
);
const originalStandaloneDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'standalone',
);
const originalServiceWorkerDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'serviceWorker',
);
const originalNotificationDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'Notification',
);
const originalIsSecureContextDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'isSecureContext',
);
const originalMatchMedia = window.matchMedia;

function restoreDescriptor(
  target: object,
  prop: string,
  descriptor: PropertyDescriptor | undefined,
) {
  if (descriptor) {
    Object.defineProperty(target, prop, descriptor);
  } else {
    Reflect.deleteProperty(target, prop);
  }
}

function setBrowserEnvironment({
  userAgent = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36',
  platform = 'Linux armv8l',
  maxTouchPoints = 5,
  standaloneMedia = false,
  navigatorStandalone,
  isSecureContext = true,
  notificationPermission = 'default',
  notificationSupported = true,
  showNotification = vi.fn().mockResolvedValue(undefined),
}: {
  userAgent?: string;
  platform?: string;
  maxTouchPoints?: number;
  standaloneMedia?: boolean;
  navigatorStandalone?: boolean;
  isSecureContext?: boolean;
  notificationPermission?: NotificationPermission;
  notificationSupported?: boolean;
  showNotification?: ReturnType<typeof vi.fn>;
} = {}) {
  Object.defineProperty(window.navigator, 'userAgent', {
    value: userAgent,
    configurable: true,
  });
  Object.defineProperty(window.navigator, 'platform', {
    value: platform,
    configurable: true,
  });
  Object.defineProperty(window.navigator, 'maxTouchPoints', {
    value: maxTouchPoints,
    configurable: true,
  });

  if (navigatorStandalone !== undefined) {
    Object.defineProperty(window.navigator, 'standalone', {
      value: navigatorStandalone,
      configurable: true,
    });
  } else {
    Reflect.deleteProperty(window.navigator, 'standalone');
  }

  Object.defineProperty(window, 'isSecureContext', {
    value: isSecureContext,
    configurable: true,
  });

  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === '(display-mode: standalone)' ? standaloneMedia : false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));

  const fakeRegistration = {
    showNotification,
    pushManager: {
      getSubscription: vi.fn().mockResolvedValue(null),
      subscribe: vi.fn(),
    },
  };

  Object.defineProperty(window.navigator, 'serviceWorker', {
    value: {
      register: vi.fn().mockResolvedValue(fakeRegistration),
      getRegistration: vi.fn().mockResolvedValue(fakeRegistration),
    },
    configurable: true,
  });

  if (!notificationSupported) {
    Reflect.deleteProperty(window, 'Notification');
    return { requestPermission: null, showNotification };
  }

  let permissionState: NotificationPermission = notificationPermission;
  const requestPermission = vi.fn().mockImplementation(async () => {
    permissionState = 'granted';
    return permissionState;
  });

  class MockNotification {
    static get permission(): NotificationPermission {
      return permissionState;
    }

    static requestPermission = requestPermission;
  }

  Object.defineProperty(window, 'Notification', {
    value: MockNotification,
    configurable: true,
  });

  return { requestPermission, showNotification };
}

function renderWithPwa(ui: ReactNode, initialEntry = '/') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialEntry]}>
        <ToastProvider>
          <PwaProvider>{ui}</PwaProvider>
        </ToastProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  resetPwaStoreForTests();
  localStorage.clear();
  settingsMocks.getSettings.mockReset();
  settingsMocks.bulkUpdateSettings.mockReset();
  authMocks.useAuth.mockReset().mockReturnValue({
    user: null,
    isAuthenticated: false,
    isLoading: false,
  });
  notificationApiMocks.getNotificationCount
    .mockReset()
    .mockResolvedValue({ unread: 0, total: 0 });
  notificationApiMocks.getNotifications.mockReset().mockResolvedValue([]);
  notificationApiMocks.getPushConfig
    .mockReset()
    .mockResolvedValue({ enabled: false, vapid_public_key: null });
  notificationApiMocks.savePushSubscription
    .mockReset()
    .mockResolvedValue({ subscribed: true, active_subscriptions: 1 });
  notificationApiMocks.deletePushSubscription
    .mockReset()
    .mockResolvedValue({ subscribed: false, active_subscriptions: 0 });
  notificationApiMocks.sendTestPush
    .mockReset()
    .mockResolvedValue({ queued: 1, sent: 1, failed: 0, gone: 0 });
});

afterEach(() => {
  resetPwaStoreForTests();
  restoreDescriptor(window.navigator, 'userAgent', originalUserAgentDescriptor);
  restoreDescriptor(window.navigator, 'platform', originalPlatformDescriptor);
  restoreDescriptor(
    window.navigator,
    'maxTouchPoints',
    originalMaxTouchPointsDescriptor,
  );
  restoreDescriptor(
    window.navigator,
    'standalone',
    originalStandaloneDescriptor,
  );
  restoreDescriptor(
    window.navigator,
    'serviceWorker',
    originalServiceWorkerDescriptor,
  );
  restoreDescriptor(window, 'Notification', originalNotificationDescriptor);
  restoreDescriptor(
    window,
    'isSecureContext',
    originalIsSecureContextDescriptor,
  );
  window.matchMedia = originalMatchMedia;
});

describe('detectPwaPlatform', () => {
  it('identifies iPhone Safari, iPadOS desktop UA, iPhone Chrome, Android Chrome, and Desktop Chrome', () => {
    expect(
      detectPwaPlatform(
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
        'iPhone',
        5,
      ),
    ).toBe('ios-safari');

    expect(
      detectPwaPlatform(
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15',
        'MacIntel',
        5,
      ),
    ).toBe('ios-safari');

    expect(
      detectPwaPlatform(
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/130.0.6723.90 Mobile/15E148 Safari/604.1',
        'iPhone',
        5,
      ),
    ).toBe('ios-other');

    expect(
      detectPwaPlatform(
        'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36',
        'Linux armv8l',
        5,
      ),
    ).toBe('android');

    expect(
      detectPwaPlatform(
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
        'MacIntel',
        0,
      ),
    ).toBe('desktop');
  });
});

describe('getNotificationStatus', () => {
  it('returns ios-install-required for iOS outside standalone even when Notification is undefined', () => {
    setBrowserEnvironment({ notificationSupported: false });
    expect(getNotificationStatus('ios-safari', false)).toBe(
      'ios-install-required',
    );
    expect(getNotificationStatus('ios-other', false)).toBe(
      'ios-install-required',
    );
  });

  it('returns permission states for iOS standalone and Android/Desktop', () => {
    setBrowserEnvironment({ notificationPermission: 'default' });
    expect(getNotificationStatus('ios-safari', true)).toBe('default');
    expect(getNotificationStatus('android', false)).toBe('default');

    setBrowserEnvironment({ notificationPermission: 'granted' });
    expect(getNotificationStatus('desktop', false)).toBe('granted');

    setBrowserEnvironment({ notificationPermission: 'denied' });
    expect(getNotificationStatus('android', true)).toBe('denied');
  });

  it('returns insecure-context and unsupported appropriately', () => {
    setBrowserEnvironment({ isSecureContext: false });
    expect(getNotificationStatus('android', false)).toBe('insecure-context');

    setBrowserEnvironment({
      isSecureContext: true,
      notificationSupported: false,
    });
    expect(getNotificationStatus('desktop', false)).toBe('unsupported');
  });
});

describe('NotificationActivationCard', () => {
  it('activates notifications on Allow Notifications click and sends a test notification', async () => {
    const user = userEvent.setup();
    const showNotification = vi.fn().mockResolvedValue(undefined);
    const { requestPermission } = setBrowserEnvironment({
      notificationPermission: 'default',
      showNotification,
    });

    renderWithPwa(<NotificationActivationCard />);

    const allowBtn = screen.getByRole('button', {
      name: 'Allow Notifications',
    });
    await user.click(allowBtn);

    await waitFor(() => expect(requestPermission).toHaveBeenCalledOnce());
    expect(
      await screen.findByText('Notifications Enabled'),
    ).toBeInTheDocument();

    showNotification.mockClear();
    const testBtn = screen.getByRole('button', {
      name: 'Send Test Notification',
    });
    await user.click(testBtn);

    await waitFor(() =>
      expect(showNotification).toHaveBeenCalledWith(
        'PayrollMY Test Notification',
        expect.objectContaining({
          icon: '/icons/icon-192.png',
          badge: '/icons/badge-72.png',
        }),
      ),
    );
  });

  it('shows blocked state and hides enable button when permission is denied', async () => {
    setBrowserEnvironment({ notificationPermission: 'denied' });
    renderWithPwa(<NotificationActivationCard />);

    expect(screen.getByText('Blocked in Browser')).toBeInTheDocument();
    expect(
      screen.getByText(/Notifications are blocked for this site/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Allow Notifications' }),
    ).not.toBeInTheDocument();

    await waitFor(() =>
      expect(window.navigator.serviceWorker.register).toHaveBeenCalledOnce(),
    );
  });

  it('requires Home Screen install on iOS Safari and opens InstallAppModal with Safari steps', async () => {
    const user = userEvent.setup();
    setBrowserEnvironment({
      userAgent:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
      platform: 'iPhone',
      notificationSupported: false,
      standaloneMedia: false,
    });

    renderWithPwa(<NotificationActivationCard />);

    expect(screen.getByText('Home Screen Required')).toBeInTheDocument();
    await user.click(
      screen.getByRole('button', { name: 'Add to Home Screen' }),
    );

    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('Tap the Share button');
    expect(dialog).toHaveTextContent('Select "Add to Home Screen"');
  });
});

describe('PwaInstallBanner and InstallAppModal', () => {
  it('shows install banner on Android beforeinstallprompt, invokes prompt(), and hides after acceptance', async () => {
    const user = userEvent.setup();
    setBrowserEnvironment({
      userAgent:
        'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36',
      standaloneMedia: false,
    });

    renderWithPwa(<PwaInstallBanner />);
    expect(
      screen.queryByRole('region', { name: 'Install PayrollMY app' }),
    ).not.toBeInTheDocument();

    const promptSpy = vi.fn().mockResolvedValue(undefined);
    const installEvent = new Event(
      'beforeinstallprompt',
    ) as unknown as BeforeInstallPromptEvent;
    Object.defineProperties(installEvent, {
      platforms: { value: ['web'] },
      userChoice: {
        value: Promise.resolve({ outcome: 'accepted', platform: 'web' }),
      },
      prompt: { value: promptSpy },
    });

    act(() => {
      window.dispatchEvent(installEvent);
    });

    expect(
      await screen.findByRole('region', { name: 'Install PayrollMY app' }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Install App' }));

    await waitFor(() => expect(promptSpy).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(
        screen.queryByRole('region', { name: 'Install PayrollMY app' }),
      ).not.toBeInTheDocument(),
    );
  });

  it('shows banner on iPhone Safari, opens modal instructions, and persists dismiss snooze', async () => {
    const user = userEvent.setup();
    setBrowserEnvironment({
      userAgent:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
      platform: 'iPhone',
      standaloneMedia: false,
    });

    renderWithPwa(<PwaInstallBanner />);

    expect(
      screen.getByRole('region', { name: 'Install PayrollMY app' }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole('button', { name: 'Add to Home Screen' }),
    );
    expect(await screen.findByRole('dialog')).toHaveTextContent(
      'Tap the Share button',
    );
    await user.click(screen.getByRole('button', { name: 'Close' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    );

    await user.click(
      screen.getByRole('button', { name: 'Dismiss install prompt' }),
    );
    expect(
      localStorage.getItem('payroll-pwa-install-dismissed-at'),
    ).not.toBeNull();
    expect(
      screen.queryByRole('region', { name: 'Install PayrollMY app' }),
    ).not.toBeInTheDocument();
  });

  it('suppresses the install banner in standalone mode', async () => {
    setBrowserEnvironment({
      userAgent:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
      platform: 'iPhone',
      standaloneMedia: true,
    });

    renderWithPwa(<PwaInstallBanner />);
    expect(
      screen.queryByRole('region', { name: 'Install PayrollMY app' }),
    ).not.toBeInTheDocument();

    await waitFor(() =>
      expect(window.navigator.serviceWorker.register).toHaveBeenCalledOnce(),
    );
  });
});

describe('SettingsPage App & Alerts section', () => {
  it('renders AppInstallCard and NotificationActivationCard for ?section=app', async () => {
    setBrowserEnvironment({ notificationPermission: 'default' });
    const sampleSetting: CompanySetting = {
      id: 's-1',
      company_id: 'company-1',
      category: 'system',
      key: 'dark_mode',
      value: false,
      label: 'Dark mode',
      description: null,
      updated_at: '2026-01-01T00:00:00Z',
      updated_by: null,
    };
    settingsMocks.getSettings.mockResolvedValue([sampleSetting]);

    renderWithPwa(<SettingsPage />, '/settings?section=app');

    expect(await screen.findByText('Home Screen App')).toBeInTheDocument();
    expect(screen.getByText('Device Notifications')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Allow Notifications' }),
    ).toBeInTheDocument();
  });
});

describe('PWA public assets & manifest integrity', () => {
  it('ships valid manifest.webmanifest, icons, sw.js, and offline.html', () => {
    const publicDir = path.resolve(import.meta.dirname, '../../public');
    const manifestPath = path.join(publicDir, 'manifest.webmanifest');
    expect(fs.existsSync(manifestPath)).toBe(true);

    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as {
      display: string;
      start_url: string;
      icons: { src: string; sizes: string; purpose?: string }[];
    };

    expect(manifest.display).toBe('standalone');
    expect(manifest.start_url).toBe('/');
    expect(manifest.icons).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ sizes: '192x192', purpose: 'any' }),
        expect.objectContaining({ sizes: '512x512', purpose: 'any' }),
        expect.objectContaining({ sizes: '192x192', purpose: 'maskable' }),
        expect.objectContaining({ sizes: '512x512', purpose: 'maskable' }),
      ]),
    );

    for (const icon of manifest.icons) {
      const relativeIconPath = icon.src.replace(/^\//, '');
      expect(fs.existsSync(path.join(publicDir, relativeIconPath))).toBe(true);
    }

    expect(
      fs.existsSync(path.join(publicDir, 'icons/apple-touch-icon.png')),
    ).toBe(true);
    expect(fs.existsSync(path.join(publicDir, 'icons/badge-72.png'))).toBe(
      true,
    );
    expect(fs.existsSync(path.join(publicDir, 'sw.js'))).toBe(true);
    expect(fs.existsSync(path.join(publicDir, 'offline.html'))).toBe(true);
  });
});

describe('Web Push subscription & Service Worker runtime', () => {
  const VAPID_KEY_A =
    'BJvxi1WNriIrivViVml_qavLQzTjsf15Owr4ABMG6BkHOYT8-ACmAaf1dr2LhwGfasF3EfqzDL-tmRNRLvXg-vQ';
  const VAPID_KEY_B =
    'BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4';

  it('subscribes and rotates PushSubscription when VAPID public key changes', async () => {
    const oldUnsubscribe = vi.fn().mockResolvedValue(true);
    const oldSub = {
      endpoint: 'https://fcm.googleapis.com/fcm/send/old',
      options: {
        applicationServerKey: urlBase64ToUint8Array(VAPID_KEY_A).buffer,
      },
      unsubscribe: oldUnsubscribe,
      toJSON: () => ({
        endpoint: 'https://fcm.googleapis.com/fcm/send/old',
        keys: { p256dh: 'p256-old', auth: 'auth-old' },
      }),
    } as unknown as PushSubscription;

    const newSub = {
      endpoint: 'https://fcm.googleapis.com/fcm/send/new',
      options: {
        applicationServerKey: urlBase64ToUint8Array(VAPID_KEY_B).buffer,
      },
      unsubscribe: vi.fn().mockResolvedValue(true),
      toJSON: () => ({
        endpoint: 'https://fcm.googleapis.com/fcm/send/new',
        keys: { p256dh: 'p256-new', auth: 'auth-new' },
      }),
    } as unknown as PushSubscription;

    const pushManager = {
      getSubscription: vi
        .fn()
        .mockResolvedValueOnce(oldSub)
        .mockResolvedValueOnce(oldSub),
      subscribe: vi.fn().mockResolvedValue(newSub),
    };
    const reg = { pushManager } as unknown as ServiceWorkerRegistration;

    const sameKeyResult = await subscribeToPush(reg, VAPID_KEY_A);
    expect(sameKeyResult?.endpoint).toBe('https://fcm.googleapis.com/fcm/send/old');
    expect(oldUnsubscribe).not.toHaveBeenCalled();

    const rotatedResult = await subscribeToPush(reg, VAPID_KEY_B);
    expect(oldUnsubscribe).toHaveBeenCalledOnce();
    expect(pushManager.subscribe).toHaveBeenCalledOnce();
    expect(rotatedResult).toEqual({
      endpoint: 'https://fcm.googleapis.com/fcm/send/new',
      keys: { p256dh: 'p256-new', auth: 'auth-new' },
    });
  });

  it('syncs PushSubscription when authenticated and sends real server test push', async () => {
    const user = userEvent.setup();
    const fakeSub = {
      endpoint: 'https://fcm.googleapis.com/fcm/send/device-1',
      options: {
        applicationServerKey: urlBase64ToUint8Array(VAPID_KEY_A).buffer,
      },
      unsubscribe: vi.fn().mockResolvedValue(true),
      toJSON: () => ({
        endpoint: 'https://fcm.googleapis.com/fcm/send/device-1',
        keys: { p256dh: 'p256-device-1', auth: 'auth-device-1' },
      }),
    } as unknown as PushSubscription;

    setBrowserEnvironment({ notificationPermission: 'granted' });
    const reg = await navigator.serviceWorker.getRegistration('/');
    if (reg) {
      vi.mocked(reg.pushManager.getSubscription).mockResolvedValue(null);
      vi.mocked(reg.pushManager.subscribe).mockResolvedValue(fakeSub);
    }

    authMocks.useAuth.mockReturnValue({
      user: { id: 'u-1', company_id: 'c-1', must_change_password: false },
      isAuthenticated: true,
      isLoading: false,
    });
    notificationApiMocks.getPushConfig.mockResolvedValue({
      enabled: true,
      vapid_public_key: VAPID_KEY_A,
    });

    renderWithPwa(<NotificationActivationCard />);

    await waitFor(() =>
      expect(notificationApiMocks.savePushSubscription).toHaveBeenCalledWith({
        endpoint: 'https://fcm.googleapis.com/fcm/send/device-1',
        keys: { p256dh: 'p256-device-1', auth: 'auth-device-1' },
      }),
    );
    expect(await screen.findByText('Push Active')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Send Test Notification' }));
    await waitFor(() => expect(notificationApiMocks.sendTestPush).toHaveBeenCalledOnce());
  });

  it('updates and clears app icon badges via Badging API', () => {
    const setAppBadge = vi.fn().mockResolvedValue(undefined);
    const clearAppBadge = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(window.navigator, 'setAppBadge', {
      value: setAppBadge,
      configurable: true,
    });
    Object.defineProperty(window.navigator, 'clearAppBadge', {
      value: clearAppBadge,
      configurable: true,
    });

    updateAppBadge(4);
    expect(setAppBadge).toHaveBeenCalledWith(4);
    updateAppBadge(0);
    expect(clearAppBadge).toHaveBeenCalledOnce();
  });

  it('handles push, notificationclick sanitization, and pushsubscriptionchange in sw.js', async () => {
    const swPath = path.resolve(import.meta.dirname, '../../public/sw.js');
    const swSource = fs.readFileSync(swPath, 'utf8');

    const listeners = new Map<string, (event: unknown) => void>();
    const showNotification = vi.fn().mockResolvedValue(undefined);
    const setAppBadge = vi.fn().mockResolvedValue(undefined);
    const openWindow = vi.fn().mockResolvedValue(undefined);
    const postMessage = vi.fn();
    const swSubscribe = vi.fn().mockResolvedValue({});

    const fakeSelf = {
      location: new URL('https://payrollmy.com/sw.js'),
      skipWaiting: vi.fn().mockResolvedValue(undefined),
      navigator: {
        setAppBadge,
        clearAppBadge: vi.fn().mockResolvedValue(undefined),
      },
      registration: {
        showNotification,
        pushManager: { subscribe: swSubscribe },
      },
      clients: {
        claim: vi.fn().mockResolvedValue(undefined),
        matchAll: vi.fn().mockResolvedValue([{ url: 'https://payrollmy.com/dashboard', postMessage }]),
        openWindow,
      },
      addEventListener: (name: string, fn: (event: unknown) => void) => {
        listeners.set(name, fn);
      },
    };

    const runSw = new Function('self', 'caches', 'fetch', swSource);
    runSw(
      fakeSelf,
      { open: vi.fn(), keys: vi.fn(), match: vi.fn() },
      vi.fn(),
    );

    // 1. push event renders notification and sets app badge
    const pushWaits: Promise<unknown>[] = [];
    listeners.get('push')?.({
      data: {
        json: () => ({
          title: 'Payslip Ready',
          body: 'Your 11/2026 payslip is ready.',
          url: 'https://evil.example/phish',
          tag: 'payroll-1',
          unread_count: 3,
        }),
      },
      waitUntil: (p: Promise<unknown>) => pushWaits.push(p),
    });
    await Promise.all(pushWaits);

    expect(showNotification).toHaveBeenCalledWith(
      'Payslip Ready',
      expect.objectContaining({
        body: 'Your 11/2026 payslip is ready.',
        icon: '/icons/icon-192.png',
        badge: '/icons/badge-72.png',
        tag: 'payroll-1',
        renotify: true,
        // Cross-origin URL must be rejected and replaced with safe default
        data: { url: '/portal/notifications' },
      }),
    );
    expect(setAppBadge).toHaveBeenCalledWith(3);

    // 2. notificationclick navigates existing same-origin client
    const navigateSpy = vi.fn().mockResolvedValue(undefined);
    const focusSpy = vi.fn().mockResolvedValue(undefined);
    fakeSelf.clients.matchAll.mockResolvedValueOnce([
      {
        url: 'https://payrollmy.com/portal/profile',
        navigate: navigateSpy,
        focus: focusSpy,
      },
    ]);
    const clickWaits: Promise<unknown>[] = [];
    const closeSpy = vi.fn();
    listeners.get('notificationclick')?.({
      notification: { close: closeSpy, data: { url: '/portal/payslips' } },
      waitUntil: (p: Promise<unknown>) => clickWaits.push(p),
    });
    await Promise.all(clickWaits);

    expect(closeSpy).toHaveBeenCalledOnce();
    expect(navigateSpy).toHaveBeenCalledWith('https://payrollmy.com/portal/payslips');
    expect(focusSpy).toHaveBeenCalledOnce();

    // 3. pushsubscriptionchange re-subscribes and notifies foreground clients
    const subChangeWaits: Promise<unknown>[] = [];
    listeners.get('pushsubscriptionchange')?.({
      oldSubscription: {
        options: { applicationServerKey: new Uint8Array([4, 1, 2, 3]) },
      },
      waitUntil: (p: Promise<unknown>) => subChangeWaits.push(p),
    });
    await Promise.all(subChangeWaits);

    expect(swSubscribe).toHaveBeenCalledOnce();
    expect(postMessage).toHaveBeenCalledWith({ type: 'PUSH_SUBSCRIPTION_CHANGED' });
  });
});
