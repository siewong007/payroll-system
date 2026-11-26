import { Download, Smartphone, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { usePwa } from '@/context/PwaContext';

export function PwaInstallBanner() {
  const { t } = useTranslation();
  const {
    isIos,
    isStandalone,
    canPromptInstall,
    bannerDismissed,
    dismissBanner,
    triggerInstall,
  } = usePwa();

  if (isStandalone || bannerDismissed || (!canPromptInstall && !isIos)) {
    return null;
  }

  return (
    <div
      role="region"
      aria-label={t('pwa.banner.regionLabel')}
      className="no-print mb-5 rounded-2xl border border-[var(--accent-1)]/20 bg-white/90 dark:bg-slate-900/90 px-3.5 py-2.5 sm:px-4 sm:py-3 shadow-sm flex flex-col sm:flex-row sm:items-center gap-3"
    >
      <div className="flex items-center gap-3 min-w-0 flex-1">
        <img
          src="/icons/icon-192.png"
          alt=""
          width={36}
          height={36}
          className="h-9 w-9 rounded-xl shadow-xs shrink-0"
        />
        <div className="min-w-0">
          <p className="text-sm font-semibold text-gray-900 dark:text-slate-100 truncate">
            {t('pwa.banner.title')}
          </p>
          <p className="text-xs text-gray-500 dark:text-slate-400">
            {canPromptInstall
              ? t('pwa.banner.subtitleInstallable')
              : t('pwa.banner.subtitleIos')}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2 self-end sm:self-center shrink-0">
        <button
          type="button"
          onClick={() => void triggerInstall()}
          className="btn-primary !min-h-10 !py-2 !px-3.5 text-xs"
        >
          {canPromptInstall ? (
            <Download className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <Smartphone className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {canPromptInstall
            ? t('pwa.actions.installApp')
            : t('pwa.actions.addToHomeScreen')}
        </button>

        <button
          type="button"
          onClick={dismissBanner}
          aria-label={t('pwa.banner.dismiss')}
          className="min-h-10 min-w-10 inline-flex items-center justify-center rounded-xl text-gray-400 hover:text-gray-600 hover:bg-gray-100 dark:hover:text-slate-200 dark:hover:bg-slate-800 transition-colors"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
