import {
  Bell,
  BellOff,
  BellRing,
  CheckCircle2,
  RefreshCw,
  Send,
  Smartphone,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { usePwa } from '@/context/PwaContext';
import type { NotificationStatus } from '@/lib/pwa';

const STATUS_BADGE_STYLES: Record<NotificationStatus, string> = {
  default:
    'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
  granted:
    'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300',
  denied:
    'bg-rose-100 text-rose-700 dark:bg-rose-500/20 dark:text-rose-300',
  'ios-install-required':
    'bg-amber-100 text-amber-800 dark:bg-amber-500/20 dark:text-amber-300',
  'insecure-context':
    'bg-amber-100 text-amber-800 dark:bg-amber-500/20 dark:text-amber-300',
  unsupported:
    'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-400',
};

const STATUS_KEY: Record<
  NotificationStatus,
  { badge: string; desc: string }
> = {
  default: {
    badge: 'pwa.notifications.statusDefault',
    desc: 'pwa.notifications.defaultDesc',
  },
  granted: {
    badge: 'pwa.notifications.statusGranted',
    desc: 'pwa.notifications.grantedDesc',
  },
  denied: {
    badge: 'pwa.notifications.statusDenied',
    desc: 'pwa.notifications.deniedDesc',
  },
  'ios-install-required': {
    badge: 'pwa.notifications.statusIosInstallRequired',
    desc: 'pwa.notifications.iosInstallDesc',
  },
  'insecure-context': {
    badge: 'pwa.notifications.statusInsecureContext',
    desc: 'pwa.notifications.insecureDesc',
  },
  unsupported: {
    badge: 'pwa.notifications.statusUnsupported',
    desc: 'pwa.notifications.unsupportedDesc',
  },
};

export function NotificationActivationCard() {
  const { t } = useTranslation();
  const {
    notificationStatus,
    notificationBusy,
    pushServerEnabled,
    pushSubscribed,
    enableNotifications,
    sendTestNotification,
    syncPushSubscription,
    openInstallModal,
  } = usePwa();
  const [syncingPush, setSyncingPush] = useState(false);

  const Icon =
    notificationStatus === 'granted'
      ? BellRing
      : notificationStatus === 'denied' || notificationStatus === 'unsupported'
        ? BellOff
        : Bell;

  const statusMeta = STATUS_KEY[notificationStatus];

  const badgeText =
    notificationStatus === 'granted' && pushServerEnabled && pushSubscribed
      ? t('pwa.notifications.statusPushActive')
      : t(statusMeta.badge);

  const descriptionText =
    notificationStatus === 'granted'
      ? pushServerEnabled
        ? pushSubscribed
          ? t('pwa.notifications.pushActiveDesc')
          : t('pwa.notifications.pushSyncPendingDesc')
        : t('pwa.notifications.localOnlyDesc')
      : t(statusMeta.desc);

  const handleSyncPush = async () => {
    setSyncingPush(true);
    try {
      await syncPushSubscription();
    } finally {
      setSyncingPush(false);
    }
  };

  return (
    <section className="card">
      <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-4">
        <div className="flex items-start gap-3.5 min-w-0">
          <div className="w-10 h-10 rounded-xl bg-[var(--accent-soft)] flex items-center justify-center shrink-0">
            <Icon
              className="w-5 h-5 text-[var(--accent-1)]"
              aria-hidden="true"
            />
          </div>
          <div className="min-w-0 space-y-1.5">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="section-title">{t('pwa.notifications.cardTitle')}</h2>
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${STATUS_BADGE_STYLES[notificationStatus]}`}
              >
                {notificationStatus === 'granted' && (
                  <CheckCircle2 className="h-3.5 w-3.5" aria-hidden="true" />
                )}
                {badgeText}
              </span>
            </div>
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('pwa.notifications.cardDesc')}
            </p>
            <p className="text-xs text-gray-500 dark:text-slate-400">
              {descriptionText}
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 shrink-0">
          {notificationStatus === 'default' && (
            <button
              type="button"
              disabled={notificationBusy}
              onClick={() => void enableNotifications()}
              className="btn-primary whitespace-nowrap"
            >
              <Bell className="h-4 w-4" aria-hidden="true" />
              {notificationBusy
                ? t('pwa.notifications.enabling')
                : t('pwa.actions.allowNotifications')}
            </button>
          )}

          {notificationStatus === 'granted' && pushServerEnabled && !pushSubscribed && (
            <button
              type="button"
              disabled={syncingPush}
              onClick={() => void handleSyncPush()}
              className="btn-primary whitespace-nowrap"
            >
              <RefreshCw
                className={`h-4 w-4 ${syncingPush ? 'animate-spin' : ''}`}
                aria-hidden="true"
              />
              {t('pwa.actions.connectPush')}
            </button>
          )}

          {notificationStatus === 'granted' && (
            <button
              type="button"
              onClick={() => void sendTestNotification()}
              className="btn-secondary whitespace-nowrap"
            >
              <Send className="h-4 w-4" aria-hidden="true" />
              {t('pwa.actions.sendTestNotification')}
            </button>
          )}

          {notificationStatus === 'ios-install-required' && (
            <button
              type="button"
              onClick={openInstallModal}
              className="btn-primary whitespace-nowrap"
            >
              <Smartphone className="h-4 w-4" aria-hidden="true" />
              {t('pwa.actions.addToHomeScreen')}
            </button>
          )}
        </div>
      </div>
    </section>
  );
}
