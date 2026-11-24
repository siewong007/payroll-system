import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { PwaContext, type InstallOutcome, type PwaContextValue } from './PwaContext';
import {
  clearDeferredInstallPrompt,
  detectPwaPlatform,
  dismissInstallBanner,
  getDeferredInstallPrompt,
  getNotificationStatus,
  initPwaListeners,
  isInstallBannerDismissed,
  isIosPlatform,
  isRunningStandalone,
  registerServiceWorker,
  requestNotificationPermission,
  showLocalNotification,
  subscribeInstallPrompt,
  subscribeToPushIfConfigured,
  type NotificationStatus,
} from '@/lib/pwa';
import { toast } from '@/lib/toast';
import { InstallAppModal } from '@/components/pwa/InstallAppModal';

export function PwaProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();

  const [platform] = useState(() => detectPwaPlatform());
  const isIos = isIosPlatform(platform);

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

  const syncEnvironment = useCallback(() => {
    const standalone = isRunningStandalone();
    setIsStandalone(standalone);
    setCanPromptInstall(getDeferredInstallPrompt() !== null);
    setNotificationStatus(getNotificationStatus(platform, standalone));
  }, [platform]);

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
    void registerServiceWorker().then((registration) => {
      if (mounted && registration) {
        setSwReady(true);
      }
    });

    return () => {
      mounted = false;
      unsubscribePrompt();
      window.removeEventListener('appinstalled', handleAppInstalled);
      window.removeEventListener('focus', syncEnvironment);
      document.removeEventListener('visibilitychange', handleVisibilityOrFocus);
      media?.removeEventListener?.('change', handleMediaChange);
    };
  }, [platform, syncEnvironment]);

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
        const registration = await registerServiceWorker();
        if (registration) {
          setSwReady(true);
          void subscribeToPushIfConfigured(registration);
        }
        toast.success(t('pwa.notifications.enabledToast'));
        await showLocalNotification(t('pwa.notifications.welcomeTitle'), {
          body: t('pwa.notifications.welcomeBody'),
          tag: 'payrollmy-enabled',
          url: '/portal/notifications',
        });
      } else if (permission === 'denied') {
        toast.warning(t('pwa.notifications.blockedToast'));
      }

      return nextStatus;
    } finally {
      setNotificationBusy(false);
    }
  }, [isStandalone, platform, t]);

  const sendTestNotification = useCallback(async (): Promise<boolean> => {
    const sent = await showLocalNotification(t('pwa.notifications.testTitle'), {
      body: t('pwa.notifications.testBody'),
      tag: 'payrollmy-test',
      url: '/portal/notifications',
    });
    if (sent) {
      toast.success(t('pwa.notifications.testSentToast'));
    } else {
      toast.error(t('pwa.notifications.testFailedToast'));
    }
    return sent;
  }, [t]);

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
      openInstallModal,
      closeInstallModal,
      dismissBanner,
      triggerInstall,
      enableNotifications,
      sendTestNotification,
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
      openInstallModal,
      closeInstallModal,
      dismissBanner,
      triggerInstall,
      enableNotifications,
      sendTestNotification,
    ],
  );

  return (
    <PwaContext.Provider value={value}>
      {children}
      <InstallAppModal />
    </PwaContext.Provider>
  );
}
