import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useAuth } from './AuthContext';
import { PwaContext, type InstallOutcome, type PwaContextValue } from './PwaContext';
import {
  getNotificationCount,
  getNotifications,
  getPushConfig,
  savePushSubscription,
  sendTestPush,
} from '@/api/notifications';
import {
  clearDeferredInstallPrompt,
  detectPwaPlatform,
  dismissInstallBanner,
  getDeferredInstallPrompt,
  getNotificationStatus,
  initPwaListeners,
  isInstallBannerDismissed,
  isIosPlatform,
  isPushSupported,
  isRunningStandalone,
  registerServiceWorkerOnLoad,
  requestNotificationPermission,
  showLocalNotification,
  subscribeInstallPrompt,
  subscribeToPush,
  updateAppBadge,
  waitForActiveServiceWorker,
  type NotificationStatus,
} from '@/lib/pwa';
import { toast } from '@/lib/toast';

const InstallAppModal = lazy(() =>
  import('@/components/pwa/InstallAppModal').then((m) => ({ default: m.InstallAppModal })),
);

export function PwaProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { user, isAuthenticated } = useAuth();

  const [platform] = useState(() => detectPwaPlatform());
  const isIos = isIosPlatform(platform);
  const pushSupported = isPushSupported();

  const [isStandalone, setIsStandalone] = useState(() => isRunningStandalone());
  const [canPromptInstall, setCanPromptInstall] = useState(
    () => getDeferredInstallPrompt() !== null,
  );
  const [bannerDismissed, setBannerDismissed] = useState(() =>
    isInstallBannerDismissed(),
  );
  const [installModalOpen, setInstallModalOpen] = useState(false);
  const [notificationStatus, setNotificationStatus] = useState<NotificationStatus>(
    () => getNotificationStatus(platform, isRunningStandalone()),
  );
  const [notificationBusy, setNotificationBusy] = useState(false);
  const [swReady, setSwReady] = useState(false);
  const [pushServerEnabled, setPushServerEnabled] = useState(false);
  const [vapidPublicKey, setVapidPublicKey] = useState<string | null>(null);
  const [pushSubscribed, setPushSubscribed] = useState(false);
  const prevUnreadRef = useRef<number | null>(null);

  const syncEnvironment = useCallback(() => {
    const standalone = isRunningStandalone();
    setIsStandalone(standalone);
    setCanPromptInstall(getDeferredInstallPrompt() !== null);
    setNotificationStatus(getNotificationStatus(platform, standalone));
  }, [platform]);

  const syncPushWithKey = useCallback(
    async (serverEnabled: boolean, publicKey: string | null): Promise<boolean> => {
      if (!isAuthenticated || !serverEnabled || !publicKey) {
        setPushSubscribed(false);
        return false;
      }
      if (getNotificationStatus(platform, isRunningStandalone()) !== 'granted') {
        setPushSubscribed(false);
        return false;
      }

      const registration = await waitForActiveServiceWorker();
      if (!registration) {
        return false;
      }
      setSwReady(true);

      const payload = await subscribeToPush(registration, publicKey);
      if (!payload) {
        return false;
      }

      try {
        const res = await savePushSubscription(payload);
        setPushSubscribed(res.subscribed);
        return res.subscribed;
      } catch {
        return false;
      }
    },
    [isAuthenticated, platform],
  );

  const syncPushSubscription = useCallback(async (): Promise<boolean> => {
    if (!isAuthenticated) {
      setPushSubscribed(false);
      return false;
    }

    let enabled = pushServerEnabled;
    let key = vapidPublicKey;
    if (!enabled || !key) {
      try {
        const cfg = await getPushConfig();
        enabled = cfg.enabled;
        key = cfg.vapid_public_key;
        setPushServerEnabled(enabled);
        setVapidPublicKey(key);
      } catch {
        return false;
      }
    }

    return syncPushWithKey(enabled, key);
  }, [isAuthenticated, pushServerEnabled, syncPushWithKey, vapidPublicKey]);

  useEffect(() => {
    initPwaListeners();
    syncEnvironment();

    const unsubscribePrompt = subscribeInstallPrompt(() => {
      setCanPromptInstall(getDeferredInstallPrompt() !== null);
    });

    const handleAppInstalled = () => {
      setIsStandalone(true);
      setCanPromptInstall(false);
      setInstallModalOpen(false);
      setNotificationStatus(getNotificationStatus(platform, true));
    };

    const handleVisibilityOrFocus = () => {
      if (document.visibilityState === 'visible') {
        syncEnvironment();
      }
    };

    window.addEventListener('appinstalled', handleAppInstalled);
    window.addEventListener('focus', syncEnvironment);
    document.addEventListener('visibilitychange', handleVisibilityOrFocus);

    const media =
      typeof window.matchMedia === 'function'
        ? window.matchMedia('(display-mode: standalone)')
        : null;
    const handleMediaChange = () => syncEnvironment();
    media?.addEventListener?.('change', handleMediaChange);

    let mounted = true;
    const cancelSwOnLoad = registerServiceWorkerOnLoad((registration) => {
      if (mounted && registration) {
        setSwReady(true);
      }
    });

    const handleSwMessage = (event: MessageEvent) => {
      const data = event.data as { type?: string } | null;
      if (data?.type === 'PUSH_SUBSCRIPTION_CHANGED') {
        void syncPushSubscription();
      }
    };
    const swContainer =
      typeof navigator !== 'undefined' && 'serviceWorker' in navigator
        ? navigator.serviceWorker
        : null;
    swContainer?.addEventListener?.('message', handleSwMessage);

    return () => {
      mounted = false;
      cancelSwOnLoad();
      unsubscribePrompt();
      window.removeEventListener('appinstalled', handleAppInstalled);
      window.removeEventListener('focus', syncEnvironment);
      document.removeEventListener('visibilitychange', handleVisibilityOrFocus);
      media?.removeEventListener?.('change', handleMediaChange);
      swContainer?.removeEventListener?.('message', handleSwMessage);
    };
  }, [platform, syncEnvironment, syncPushSubscription]);

  // Fetch server VAPID config & automatically sync browser PushSubscription when authenticated.
  useEffect(() => {
    if (!isAuthenticated || user?.must_change_password) {
      setPushSubscribed(false);
      prevUnreadRef.current = null;
      updateAppBadge(0);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const cfg = await getPushConfig();
        if (cancelled) return;
        setPushServerEnabled(cfg.enabled);
        setVapidPublicKey(cfg.vapid_public_key);
        if (
          cfg.enabled &&
          cfg.vapid_public_key &&
          getNotificationStatus(platform, isRunningStandalone()) === 'granted'
        ) {
          await syncPushWithKey(cfg.enabled, cfg.vapid_public_key);
        }
      } catch {
        // Ignore transient config errors; card still shows permission status.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isAuthenticated, user?.id, user?.company_id, user?.must_change_password, platform, syncPushWithKey]);

  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    refetchInterval: 30_000,
    enabled: isAuthenticated && !user?.must_change_password,
  });

  // Keep app icon badge synced and fire foreground local notification fallback
  // only when real server Web Push is not active for this device.
  useEffect(() => {
    if (!isAuthenticated || user?.must_change_password || !notifCount) {
      return;
    }
    const unread = notifCount.unread;
    updateAppBadge(unread);

    const prev = prevUnreadRef.current;
    prevUnreadRef.current = unread;
    if (
      prev !== null &&
      unread > prev &&
      notificationStatus === 'granted' &&
      !pushSubscribed
    ) {
      void getNotifications(true, 1)
        .then((items) => {
          const latest = items[0];
          if (latest) {
            return showLocalNotification(latest.title, {
              body: latest.message,
              tag: `notif-${latest.id}`,
              url: '/portal/notifications',
            });
          }
          return false;
        })
        .catch(() => undefined);
    }
  }, [isAuthenticated, notifCount, notificationStatus, pushSubscribed, user?.must_change_password]);

  const openInstallModal = useCallback(() => setInstallModalOpen(true), []);
  const closeInstallModal = useCallback(() => setInstallModalOpen(false), []);

  const dismissBanner = useCallback(() => {
    dismissInstallBanner();
    setBannerDismissed(true);
  }, []);

  const triggerInstall = useCallback(async (): Promise<InstallOutcome> => {
    if (isStandalone) {
      return 'installed';
    }

    const promptEvent = getDeferredInstallPrompt();
    if (promptEvent) {
      try {
        await promptEvent.prompt();
        const choice = await promptEvent.userChoice;
        clearDeferredInstallPrompt();
        if (choice.outcome === 'accepted') {
          setInstallModalOpen(false);
          toast.success(t('pwa.installCard.installedToast'));
          return 'accepted';
        }
        return 'dismissed';
      } catch {
        clearDeferredInstallPrompt();
        setInstallModalOpen(true);
        return 'guide';
      }
    }

    setInstallModalOpen(true);
    return 'guide';
  }, [isStandalone, t]);

  const enableNotifications = useCallback(async (): Promise<NotificationStatus> => {
    const currentStatus = getNotificationStatus(platform, isStandalone);
    setNotificationStatus(currentStatus);

    if (currentStatus === 'ios-install-required') {
      setInstallModalOpen(true);
      return currentStatus;
    }
    if (currentStatus === 'insecure-context' || currentStatus === 'unsupported') {
      return currentStatus;
    }
    if (currentStatus === 'denied') {
      toast.warning(t('pwa.notifications.blockedToast'));
      return currentStatus;
    }

    setNotificationBusy(true);
    try {
      // Request permission immediately so iOS Safari keeps the user gesture intact.
      const permission = await requestNotificationPermission();
      const nextStatus = getNotificationStatus(platform, isStandalone);
      setNotificationStatus(nextStatus);

      if (permission === 'granted') {
        const registration = await waitForActiveServiceWorker();
        if (registration) {
          setSwReady(true);
        }
        await syncPushSubscription();
        toast.success(t('pwa.notifications.enabledToast'));
        await showLocalNotification(t('pwa.notifications.welcomeTitle'), {
          body: t('pwa.notifications.welcomeBody'),
          tag: 'payrollmy-enabled',
          url: window.location.pathname.startsWith('/portal') ? '/portal/notifications' : '/',
        });
      } else if (permission === 'denied') {
        toast.warning(t('pwa.notifications.blockedToast'));
      }

      return nextStatus;
    } finally {
      setNotificationBusy(false);
    }
  }, [isStandalone, platform, syncPushSubscription, t]);

  const sendTestNotification = useCallback(async (): Promise<boolean> => {
    if (isAuthenticated && pushServerEnabled) {
      const synced = pushSubscribed || (await syncPushSubscription());
      if (synced) {
        try {
          const result = await sendTestPush();
          if (result.sent > 0) {
            toast.success(t('pwa.notifications.testSentToast'));
            return true;
          }
        } catch {
          // Fall through to local notification fallback or error toast.
        }
      }
    }

    const sent = await showLocalNotification(t('pwa.notifications.testTitle'), {
      body: t('pwa.notifications.testBody'),
      tag: 'payrollmy-test',
      url: window.location.pathname.startsWith('/portal') ? '/portal/notifications' : '/',
    });
    if (sent) {
      toast.success(t('pwa.notifications.testSentToast'));
    } else {
      toast.error(t('pwa.notifications.testFailedToast'));
    }
    return sent;
  }, [isAuthenticated, pushServerEnabled, pushSubscribed, syncPushSubscription, t]);

  const canShowInstallGuide =
    !isStandalone && (canPromptInstall || isIos || platform === 'android');

  const value = useMemo<PwaContextValue>(
    () => ({
      platform,
      isIos,
      isStandalone,
      canPromptInstall,
      canShowInstallGuide,
      bannerDismissed,
      installModalOpen,
      notificationStatus,
      notificationBusy,
      swReady,
      pushSupported,
      pushServerEnabled,
      pushSubscribed,
      openInstallModal,
      closeInstallModal,
      dismissBanner,
      triggerInstall,
      enableNotifications,
      sendTestNotification,
      syncPushSubscription,
    }),
    [
      platform,
      isIos,
      isStandalone,
      canPromptInstall,
      canShowInstallGuide,
      bannerDismissed,
      installModalOpen,
      notificationStatus,
      notificationBusy,
      swReady,
      pushSupported,
      pushServerEnabled,
      pushSubscribed,
      openInstallModal,
      closeInstallModal,
      dismissBanner,
      triggerInstall,
      enableNotifications,
      sendTestNotification,
      syncPushSubscription,
    ],
  );

  return (
    <PwaContext.Provider value={value}>
      {children}
      {installModalOpen && (
        <Suspense fallback={null}>
          <InstallAppModal />
        </Suspense>
      )}
    </PwaContext.Provider>
  );
}
