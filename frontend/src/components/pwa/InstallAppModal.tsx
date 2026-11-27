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
      <div className="space-y-4">
        <div className="flex items-center gap-3 rounded-lg bg-[var(--surface-subtle)] p-3 border border-[var(--border)]">
          <img
            src="/icons/icon-192.png"
            alt=""
            width={44}
            height={44}
            className="h-11 w-11 rounded-lg border border-[var(--border)] shrink-0"
          />
          <div className="min-w-0">
            <p className="text-[13px] font-semibold text-[var(--foreground)]">
              {t('pwa.installModal.appTitle')}
            </p>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              {t('pwa.installModal.subtitle')}
            </p>
          </div>
        </div>

        {isStandalone ? (
          <div className="rounded-lg border border-emerald-200 bg-emerald-50/80 dark:border-emerald-500/30 dark:bg-emerald-500/10 p-3.5 space-y-1.5">
            <span className="badge badge-approved gap-1">
              <CheckCircle2 className="h-3 w-3" aria-hidden="true" />
              {t('pwa.installModal.installedBadge')}
            </span>
            <p className="text-[13px] text-emerald-800 dark:text-emerald-200">
              {t('pwa.installModal.installedDesc')}
            </p>
          </div>
        ) : canPromptInstall ? (
          <div className="space-y-4">
            <ul className="space-y-2 text-[13px] text-[var(--muted-foreground)]">
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-600 dark:text-emerald-400 shrink-0"
                  aria-hidden="true"
                />
                <span>{t('pwa.installModal.benefitHomeScreen')}</span>
              </li>
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-600 dark:text-emerald-400 shrink-0"
                  aria-hidden="true"
                />
                <span>{t('pwa.installModal.benefitFullscreen')}</span>
              </li>
              <li className="flex items-start gap-2.5">
                <CheckCircle2
                  className="h-4 w-4 mt-0.5 text-emerald-600 dark:text-emerald-400 shrink-0"
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
          <div className="space-y-2.5">
            {platform === 'ios-other' && (
              <div className="flex items-start gap-2.5 rounded-lg border border-amber-200 bg-amber-50/90 dark:border-amber-500/30 dark:bg-amber-500/10 p-3 text-xs text-amber-800 dark:text-amber-200">
                <Info className="h-4 w-4 shrink-0 mt-0.5" aria-hidden="true" />
                <span>{t('pwa.installModal.iosSafariTip')}</span>
              </div>
            )}

            <ol className="space-y-2.5">
              {steps.map((step) => (
                <li
                  key={step.num}
                  className="flex items-start gap-3 rounded-lg border border-[var(--border)] bg-[var(--card)] p-3"
                >
                  <span className="section-number">
                    {step.num}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--foreground)]">
                      <step.icon
                        className="h-4 w-4 text-[var(--accent-1)] shrink-0"
                        aria-hidden="true"
                      />
                      <span>{t(step.titleKey)}</span>
                    </div>
                    <p className="mt-0.5 text-xs text-[var(--muted-foreground)]">
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
