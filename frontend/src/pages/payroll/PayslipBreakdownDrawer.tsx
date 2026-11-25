import { useEffect, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import { useQuery } from '@tanstack/react-query';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getPayslipBreakdown } from '@/api/payroll';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import { Skeleton } from '@/components/ui/Skeleton';
import type { PayrollItemDetail } from '@/types';

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function PayslipBreakdownDrawer({
  runId,
  employeeId,
  onClose,
}: {
  runId: string;
  employeeId: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const titleId = useId();
  const drawerRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['payslipBreakdown', runId, employeeId],
    queryFn: () => getPayslipBreakdown(runId, employeeId),
  });

  useEffect(() => {
    const previousFocus = document.activeElement as HTMLElement | null;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    const focusTimer = window.requestAnimationFrame(() => {
      closeButtonRef.current?.focus();
    });

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onCloseRef.current();
        return;
      }

      if (e.key !== 'Tab') return;
      const dialog = drawerRef.current;
      if (!dialog) return;

      const focusableEls = dialog.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (focusableEls.length === 0) {
        e.preventDefault();
        return;
      }

      const first = focusableEls[0];
      const last = focusableEls[focusableEls.length - 1];

      if (e.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', onKeyDown);
    return () => {
      window.cancelAnimationFrame(focusTimer);
      document.body.style.overflow = prevOverflow;
      document.removeEventListener('keydown', onKeyDown);
      previousFocus?.focus?.();
    };
  }, []);

  const earnings = data?.lines.filter((line) => line.category === 'earning') ?? [];
  const deductions = data?.lines.filter((line) => line.category === 'deduction') ?? [];

  const drawer = (
    <div className="animate-fade-in fixed inset-0 z-[55] flex justify-end bg-slate-950/50 backdrop-blur-sm" onClick={onClose}>
      <div
        ref={drawerRef}
        className="animate-slide-in-right flex h-full w-full max-w-lg flex-col overflow-y-auto bg-white dark:bg-slate-900 shadow-2xl ring-1 ring-black/5 dark:ring-white/10"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="sticky top-0 z-10 flex items-center justify-between border-b border-gray-200 dark:border-slate-800 bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-6 py-3.5">
          <div className="min-w-0 pr-2">
            <h2 id={titleId} className="font-semibold text-gray-900 dark:text-slate-100 truncate">
              {data?.employee_name ?? t('payroll.breakdown.title')}
            </h2>
            {data && <p className="text-xs text-gray-500 dark:text-slate-400">{data.employee_number}</p>}
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            onClick={onClose}
            className="inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center rounded-lg text-gray-500 hover:bg-gray-100 dark:text-slate-400 dark:hover:bg-slate-800 transition-colors"
            aria-label={t('payroll.breakdown.close')}
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>

        {isLoading && (
          <div className="space-y-4 px-6 py-6" role="status">
            <span className="sr-only">{t('payroll.breakdown.loading')}</span>
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        )}
        {isError && (
          <div className="m-6 rounded-xl border border-red-200 dark:border-red-900/60 bg-red-50/70 dark:bg-red-950/30 p-4" role="alert">
            <p className="text-sm text-red-700 dark:text-red-300">
              {getErrorMessage(error, t('payroll.breakdown.failed'))}
            </p>
            <button
              type="button"
              onClick={() => void refetch()}
              className="btn-secondary mt-3 min-h-11 sm:min-h-9"
            >
              {t('common.retry')}
            </button>
          </div>
        )}

        {data && (
          <div className="space-y-6 px-6 py-5">
            {data.lines.length === 0 ? (
              <p className="rounded-xl border border-amber-200 dark:border-amber-500/30 bg-amber-50 dark:bg-amber-500/10 px-4 py-3 text-sm text-amber-800 dark:text-amber-200">
                {t('payroll.breakdown.noStored')}
              </p>
            ) : (
              <>
                <LineSection title={t('payroll.breakdown.earnings')} lines={earnings} tone="text-gray-900 dark:text-slate-100" />
                <LineSection title={t('payroll.breakdown.deductions')} lines={deductions} tone="text-red-600 dark:text-red-400" />
              </>
            )}

            <div className="space-y-2 rounded-2xl border border-gray-200 dark:border-slate-800 bg-gray-50/80 dark:bg-slate-950/60 p-4">
              <TotalRow label={t('payroll.breakdown.grossSalary')} value={data.item.gross_salary} />
              <TotalRow label={t('payroll.breakdown.totalDeductions')} value={data.item.total_deductions} negative />
              <div className="border-t border-gray-200 dark:border-slate-800 pt-2">
                <TotalRow label={t('payroll.breakdown.netSalary')} value={data.item.net_salary} emphasis />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );

  return typeof document === 'undefined' ? drawer : createPortal(drawer, document.body);
}

function LineSection({
  title,
  lines,
  tone,
}: {
  title: string;
  lines: PayrollItemDetail[];
  tone: string;
}) {
  const { t } = useTranslation();
  if (lines.length === 0) return null;

  return (
    <div>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-slate-400">{title}</h3>
      <div className="divide-y divide-gray-100 dark:divide-slate-800 rounded-xl border border-gray-200 dark:border-slate-800">
        {lines.map((line) => (
          <div key={line.id} className="flex items-start justify-between gap-4 px-4 py-2.5">
            <div>
              <p className="text-sm text-gray-800 dark:text-slate-200">{line.description}</p>
              <div className="mt-0.5 flex gap-1.5">
                {line.is_statutory && (
                  <span className="rounded bg-blue-50 dark:bg-blue-500/15 px-1.5 py-0.5 text-[10px] font-medium uppercase text-blue-700 dark:text-blue-300">
                    {t('payroll.breakdown.statutory')}
                  </span>
                )}
                {line.category === 'earning' && line.is_taxable === false && (
                  <span className="rounded bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] font-medium uppercase text-gray-600 dark:text-slate-400">
                    {t('payroll.breakdown.nonTaxable')}
                  </span>
                )}
              </div>
            </div>
            <span className={`shrink-0 text-sm font-medium tabular-nums ${tone}`}>{formatMYR(line.amount)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function TotalRow({
  label,
  value,
  negative,
  emphasis,
}: {
  label: string;
  value: number;
  negative?: boolean;
  emphasis?: boolean;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className={`text-sm ${emphasis ? 'font-semibold text-gray-900 dark:text-slate-100' : 'text-gray-600 dark:text-slate-400'}`}>
        {label}
      </span>
      <span
        className={`text-sm font-semibold tabular-nums ${
          emphasis
            ? 'text-emerald-600 dark:text-emerald-400'
            : negative
              ? 'text-red-600 dark:text-red-400'
              : 'text-gray-900 dark:text-slate-100'
        }`}
      >
        {negative ? '-' : ''}
        {formatMYR(value)}
      </span>
    </div>
  );
}
