import { CheckCircle2, Download, Smartphone } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { usePwa } from '@/context/PwaContext';

export function AppInstallCard() {
  const { t } = useTranslation();
  const { platform, isIos, isStandalone, canPromptInstall, triggerInstall } =
    usePwa();

  const hintKey = isIos
    ? 'pwa.installCard.iosDesc'
    : platform === 'android'
      ? 'pwa.installCard.androidDesc'
      : 'pwa.installCard.desktopDesc';

  return (
    <section className="card">
      <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-4">
        <div className="flex items-start gap-3.5 min-w-0">
          <img
            src="/icons/icon-192.png"
            alt=""
            width={40}
            height={40}
            className="w-10 h-10 rounded-xl shadow-xs shrink-0"
          />
          <div className="min-w-0 space-y-1.5">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="section-title">{t('pwa.installCard.title')}</h2>
              {isStandalone && (
                <span className="inline-flex items-center gap-1 rounded-full bg-emerald-100 dark:bg-emerald-500/20 px-2.5 py-0.5 text-xs font-semibold text-emerald-700 dark:text-emerald-300">
                  <CheckCircle2 className="h-3.5 w-3.5" aria-hidden="true" />
                  {t('pwa.installCard.installedBadge')}
                </span>
              )}
            </div>
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {isStandalone
                ? t('pwa.installCard.installedDesc')
                : t(hintKey)}
            </p>
          </div>
        </div>

        {!isStandalone && (
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={() => void triggerInstall()}
              className="btn-primary whitespace-nowrap"
            >
              {canPromptInstall ? (
                <Download className="h-4 w-4" aria-hidden="true" />
              ) : (
                <Smartphone className="h-4 w-4" aria-hidden="true" />
              )}
              {canPromptInstall
                ? t('pwa.actions.installApp')
                : t('pwa.actions.addToHomeScreen')}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
