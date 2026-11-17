import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const STAGES = [
  { key: 'draft', labelKey: 'ux.payrollStepper.draft' },
  { key: 'processed', labelKey: 'ux.payrollStepper.processed' },
  { key: 'approved', labelKey: 'ux.payrollStepper.approved' },
  { key: 'paid', labelKey: 'ux.payrollStepper.paid' },
] as const;

function resolveStageIndex(status: string): number {
  switch (status) {
    case 'draft':
      return 0;
    case 'processed':
    case 'pending_approval':
      return 1;
    case 'approved':
      return 2;
    case 'paid':
      return 3;
    default:
      return -1;
  }
}

export function PayrollLifecycleStepper({ status }: { status: string }) {
  const { t } = useTranslation();
  const activeIndex = resolveStageIndex(status);
  if (activeIndex < 0) return null;

  return (
    <div className="card mb-6 !py-3.5 !px-4">
      <ol className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {STAGES.map((stage, idx) => {
          const completed = idx < activeIndex;
          const current = idx === activeIndex;
          return (
            <li key={stage.key} className="flex items-center gap-2.5 min-w-0">
              <span
                className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-bold transition-all ${
                  completed
                    ? 'bg-emerald-500 text-white shadow-xs'
                    : current
                      ? 'bg-gradient-to-br from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-[0_4px_12px_-3px_var(--glow)]'
                      : 'bg-gray-100 text-gray-400 dark:bg-slate-800 dark:text-slate-500'
                }`}
              >
                {completed ? <Check className="h-3.5 w-3.5" aria-hidden="true" /> : idx + 1}
              </span>
              <span
                className={`truncate text-xs font-medium ${
                  current
                    ? 'font-semibold text-gray-900 dark:text-slate-100'
                    : completed
                      ? 'text-emerald-700 dark:text-emerald-300'
                      : 'text-gray-400 dark:text-slate-500'
                }`}
              >
                {t(stage.labelKey)}
              </span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
