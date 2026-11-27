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
      className="no-print mb-4 rounded-lg border border-[var(--border)] bg-[var(--card)] px-3.5 py-2.5 shadow-[0_1px_2px_rgba(0,0,0,0.03)] flex flex-col sm:flex-row sm:items-center gap-3"
    >
      <div className="flex items-center gap-3 min-w-0 flex-1">
        <img
          src="/icons/icon-192.png"
          alt=""
          width={36}
          height={36}
          className="h-9 w-9 rounded-lg border border-[var(--border)] shrink-0"
        />
        <div className="min-w-0">
          <p className="text-[13px] font-semibold text-[var(--foreground)] truncate">
            {t('pwa.banner.title')}
          </p>
          <p className="text-xs text-[var(--muted-foreground)]">
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
          className="btn-primary !min-h-9 !py-1.5 !px-3 text-xs"
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
          className="min-h-9 min-w-9 inline-flex items-center justify-center rounded-md text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)] transition-colors"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
