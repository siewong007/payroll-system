import { Check, XCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const STAGES = [
  { key: 'draft', labelKey: 'ux.payrollStepper.draft' },
  { key: 'processed', labelKey: 'ux.payrollStepper.processed' },
  { key: 'pending_approval', labelKey: 'ux.payrollStepper.pendingApproval' },
  { key: 'approved', labelKey: 'ux.payrollStepper.approved' },
  { key: 'paid', labelKey: 'ux.payrollStepper.paid' },
] as const;

function resolveStageIndex(status: string): number {
  switch (status) {
    case 'draft':
      return 0;
    case 'processing':
    case 'processed':
      return 1;
    case 'pending_approval':
      return 2;
    case 'approved':
      return 3;
    case 'paid':
      return 4;
    default:
      return -1;
  }
}

export function PayrollLifecycleStepper({ status }: { status: string }) {
  const { t } = useTranslation();

  if (status === 'cancelled') {
    return (
      <div
        className="card mb-6 !py-3.5 !px-4 flex items-center gap-2.5 border-red-200/80 dark:border-red-900/50 bg-red-50/70 dark:bg-red-950/30"
        role="status"
      >
        <XCircle className="h-5 w-5 shrink-0 text-red-600 dark:text-red-400" aria-hidden="true" />
        <span className="text-sm font-semibold text-red-700 dark:text-red-300">
          {t('ux.payrollStepper.cancelled')}
        </span>
      </div>
    );
  }

  const activeIndex = resolveStageIndex(status);
  if (activeIndex < 0) return null;

  return (
    <nav className="card mb-6 !py-3.5 !px-4" aria-label={t('ux.payrollStepper.navLabel')}>
      <ol className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
        {STAGES.map((stage, idx) => {
          const completed = idx < activeIndex;
          const current = idx === activeIndex;
          return (
            <li
              key={stage.key}
              className="flex items-center gap-2.5 min-w-0"
              aria-current={current ? 'step' : undefined}
            >
              <span
                className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-bold transition-all ${
                  completed
                    ? 'bg-emerald-500 text-white shadow-xs'
                    : current
                      ? 'bg-gradient-to-br from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-[0_4px_12px_-3px_var(--glow)]'
                      : 'bg-gray-100 text-gray-500 dark:bg-slate-800 dark:text-slate-400'
                }`}
                aria-hidden="true"
              >
                {completed ? <Check className="h-3.5 w-3.5" /> : idx + 1}
              </span>
              <span
                className={`text-xs font-medium leading-snug break-words ${
                  current
                    ? 'font-semibold text-gray-900 dark:text-slate-100'
                    : completed
                      ? 'text-emerald-700 dark:text-emerald-300'
                      : 'text-gray-500 dark:text-slate-400'
                }`}
              >
                {t(stage.labelKey)}
              </span>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
