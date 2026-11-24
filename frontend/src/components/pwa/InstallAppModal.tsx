import {
  CheckCircle2,
  Download,
  Info,
  MoreVertical,
  PlusSquare,
  Share,
  Smartphone,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { usePwa } from '@/context/PwaContext';

interface StepItem {
  num: number;
  icon: typeof Share;
  titleKey: string;
  descKey: string;
}

const IOS_STEPS: StepItem[] = [
  {
    num: 1,
    icon: Share,
    titleKey: 'pwa.installModal.iosStep1Title',
    descKey: 'pwa.installModal.iosStep1Desc',
  },
  {
    num: 2,
    icon: PlusSquare,
    titleKey: 'pwa.installModal.iosStep2Title',
    descKey: 'pwa.installModal.iosStep2Desc',
  },
  {
    num: 3,
    icon: CheckCircle2,
    titleKey: 'pwa.installModal.iosStep3Title',
    descKey: 'pwa.installModal.iosStep3Desc',
  },
];

const ANDROID_STEPS: StepItem[] = [
  {
    num: 1,
    icon: MoreVertical,
    titleKey: 'pwa.installModal.androidStep1Title',
    descKey: 'pwa.installModal.androidStep1Desc',
  },
  {
    num: 2,
    icon: Smartphone,
    titleKey: 'pwa.installModal.androidStep2Title',
    descKey: 'pwa.installModal.androidStep2Desc',
  },
  {
    num: 3,
    icon: CheckCircle2,
    titleKey: 'pwa.installModal.androidStep3Title',
    descKey: 'pwa.installModal.androidStep3Desc',
  },
];

export function InstallAppModal() {
  const { t } = useTranslation();
  const {
    platform,
    isIos,
    isStandalone,
    canPromptInstall,
    installModalOpen,
    closeInstallModal,
    triggerInstall,
  } = usePwa();

  const steps = isIos ? IOS_STEPS : ANDROID_STEPS;

  return (
    <Modal
      open={installModalOpen}
      onClose={closeInstallModal}
      title={t('pwa.installModal.title')}
      maxWidth="max-w-md"
      footer={
        <div className="flex justify-end">
          <button
            type="button"
            className="btn-secondary !min-h-0 !py-2"
            onClick={closeInstallModal}
          >
            {t('common.close')}
          </button>
        </div>
      }
    >
      <div className="space-y-5">
        <div className="flex items-center gap-3.5 rounded-xl bg-slate-50/90 dark:bg-slate-800/70 p-3.5 ring-1 ring-slate-200/70 dark:ring-slate-700/60">
          <img
            src="/icons/icon-192.png"
            alt=""
            width={52}
            height={52}
            className="h-13 w-13 rounded-2xl shadow-sm shrink-0"
          />
          <div className="min-w-0">
            <p className="text-sm font-semibold text-gray-900 dark:text-slate-100">
              {t('pwa.installModal.appTitle')}
            </p>
            <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">
              {t('pwa.installModal.subtitle')}
            </p>
          </div>
        </div>

        {isStandalone ? (
          <div className="rounded-xl border border-emerald-200 bg-emerald-50/80 dark:border-emerald-500/30 dark:bg-emerald-500/10 p-4 space-y-2">
            <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-100 dark:bg-emerald-500/20 px-2.5 py-1 text-xs font-semibold text-emerald-700 dark:text-emerald-300">
              <CheckCircle2 className="h-3.5 w-3.5" aria-hidden="true" />
              {t('pwa.installModal.installedBadge')}
            </span>
            <p className="text-sm text-emerald-800 dark:text-emerald-200">
              {t('pwa.installModal.installedDesc')}
            </p>
          </div>
        ) : canPromptInstall ? (
          <div className="space-y-4">
            <ul className="space-y-2.5 text-sm text-gray-600 dark:text-slate-300">
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-500 shrink-0"
                  aria-hidden="true"
                />
                <span>{t('pwa.installModal.benefitHomeScreen')}</span>
              </li>
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-500 shrink-0"
                  aria-hidden="true"
                />
                <span>{t('pwa.installModal.benefitFullscreen')}</span>
              </li>
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-500 shrink-0"
                  aria-hidden="true"
                />
                <span>{t('pwa.installModal.benefitNotifications')}</span>
              </li>
            </ul>

            <button
              type="button"
              onClick={() => void triggerInstall()}
              className="btn-primary w-full"
            >
              <Download className="h-4 w-4" aria-hidden="true" />
              {t('pwa.actions.installApp')}
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            {platform === 'ios-other' && (
              <div className="flex items-start gap-2.5 rounded-xl border border-amber-200 bg-amber-50/90 dark:border-amber-500/30 dark:bg-amber-500/10 p-3 text-xs text-amber-800 dark:text-amber-200">
                <Info className="h-4 w-4 shrink-0 mt-0.5" aria-hidden="true" />
                <span>{t('pwa.installModal.iosSafariTip')}</span>
              </div>
            )}

            <ol className="space-y-3">
              {steps.map((step) => (
                <li
                  key={step.num}
                  className="flex items-start gap-3 rounded-xl border border-gray-200/80 dark:border-slate-800 bg-white/70 dark:bg-slate-900/60 p-3"
                >
                  <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--accent-soft)] text-xs font-bold text-[var(--accent-1)]">
                    {step.num}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5 text-sm font-semibold text-gray-900 dark:text-slate-100">
                      <step.icon
                        className="h-4 w-4 text-[var(--accent-1)] shrink-0"
                        aria-hidden="true"
                      />
                      <span>{t(step.titleKey)}</span>
                    </div>
                    <p className="mt-0.5 text-xs text-gray-500 dark:text-slate-400">
                      {t(step.descKey)}
                    </p>
                  </div>
                </li>
              ))}
            </ol>
          </div>
        )}
      </div>
    </Modal>
  );
}
