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

function NotificationActions({
  compact,
  notificationStatus,
  notificationBusy,
  pushServerEnabled,
  pushSubscribed,
  syncingPush,
  enableNotifications,
  handleSyncPush,
  sendTestNotification,
  openInstallModal,
}: {
  compact: boolean;
  notificationStatus: NotificationStatus;
  notificationBusy: boolean;
  pushServerEnabled: boolean;
  pushSubscribed: boolean;
  syncingPush: boolean;
  enableNotifications: () => Promise<unknown>;
  handleSyncPush: () => Promise<void>;
  sendTestNotification: () => Promise<unknown>;
  openInstallModal: () => void;
}) {
  const { t } = useTranslation();
  const sizeClass = compact ? 'min-h-11 sm:!min-h-9 !py-1.5 !px-3 text-xs' : '';
  const iconClass = compact ? 'h-3.5 w-3.5' : 'h-4 w-4';

  return (
    <div className="flex flex-wrap items-center gap-2 shrink-0">
      {notificationStatus === 'default' && (
        <button
          type="button"
          disabled={notificationBusy}
          onClick={() => void enableNotifications()}
          className={`btn-primary whitespace-nowrap ${sizeClass}`}
        >
          <Bell className={iconClass} aria-hidden="true" />
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
          className={`btn-primary whitespace-nowrap ${sizeClass}`}
        >
          <RefreshCw
            className={`${iconClass} ${syncingPush ? 'animate-spin' : ''}`}
            aria-hidden="true"
          />
          {t('pwa.actions.connectPush')}
        </button>
      )}

      {notificationStatus === 'granted' && (
        <button
          type="button"
          onClick={() => void sendTestNotification()}
          className={`btn-secondary whitespace-nowrap ${sizeClass}`}
        >
          <Send className={iconClass} aria-hidden="true" />
          {t('pwa.actions.sendTestNotification')}
        </button>
      )}

      {notificationStatus === 'ios-install-required' && (
        <button
          type="button"
          onClick={openInstallModal}
          className={`btn-primary whitespace-nowrap ${sizeClass}`}
        >
          <Smartphone className={iconClass} aria-hidden="true" />
          {t('pwa.actions.addToHomeScreen')}
        </button>
      )}
    </div>
  );
}

export function NotificationActivationCard({ compact = false }: { compact?: boolean }) {
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
  const isFullyActive =
    notificationStatus === 'granted' && (!pushServerEnabled || pushSubscribed);

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

  const actions = (
    <NotificationActions
      compact={compact}
      notificationStatus={notificationStatus}
      notificationBusy={notificationBusy}
      pushServerEnabled={pushServerEnabled}
      pushSubscribed={pushSubscribed}
      syncingPush={syncingPush}
      enableNotifications={enableNotifications}
      handleSyncPush={handleSyncPush}
      sendTestNotification={sendTestNotification}
      openInstallModal={openInstallModal}
    />
  );

  if (compact) {
    return (
      <section
        className={`card !p-3.5 sm:!px-5 sm:!py-3.5 @container ${
          isFullyActive
            ? 'border-emerald-200/70 dark:border-emerald-500/25'
            : 'border-[var(--accent-1)]/20'
        }`}
      >
        <div className="flex flex-col @md:flex-row @md:items-center justify-between gap-3">
          <div className="flex items-start @md:items-center gap-3 min-w-0">
            <div className="w-8 h-8 rounded-lg bg-[var(--accent-soft)] flex items-center justify-center shrink-0">
              <Icon
                className="w-4 h-4 text-[var(--accent-1)]"
                aria-hidden="true"
              />
            </div>
            <div className="min-w-0 space-y-0.5">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100">
                  {t('pwa.notifications.cardTitle')}
                </h2>
                <span
                  className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-semibold ${STATUS_BADGE_STYLES[notificationStatus]}`}
                >
                  {notificationStatus === 'granted' && (
                    <CheckCircle2 className="h-3 w-3" aria-hidden="true" />
                  )}
                  {badgeText}
                </span>
              </div>
              <p
                className={`text-xs text-gray-500 dark:text-slate-400 ${
                  isFullyActive ? 'hidden sm:block' : ''
                }`}
              >
                {descriptionText}
              </p>
            </div>
          </div>

          {actions}
        </div>
      </section>
    );
  }

  return (
    <section className="card @container">
      <div className="flex flex-col @lg:flex-row @lg:items-center justify-between gap-4">
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

        {actions}
      </div>
    </section>
  );
}
