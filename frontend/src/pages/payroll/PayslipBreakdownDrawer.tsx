import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useQuery } from '@tanstack/react-query';
import { motion, useReducedMotion } from 'framer-motion';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getPayslipBreakdown } from '@/api/payroll';
import { formatMYR } from '@/lib/utils';
import { Skeleton } from '@/components/ui/Skeleton';
import type { PayrollItemDetail } from '@/types';

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
  const prefersReduced = useReducedMotion();
  const { data, isLoading, isError } = useQuery({
    queryKey: ['payslipBreakdown', runId, employeeId],
    queryFn: () => getPayslipBreakdown(runId, employeeId),
  });

  useEffect(() => {
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.body.style.overflow = prevOverflow;
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [onClose]);

  const earnings = data?.lines.filter((line) => line.category === 'earning') ?? [];
  const deductions = data?.lines.filter((line) => line.category === 'deduction') ?? [];

  const drawer = (
    <div className="fixed inset-0 z-[55] flex justify-end bg-slate-950/50 backdrop-blur-sm" onClick={onClose}>
      <motion.div
        initial={prefersReduced ? { opacity: 0 } : { x: '100%' }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ type: 'spring', stiffness: 380, damping: 34 }}
        className="flex h-full w-full max-w-lg flex-col overflow-y-auto bg-white/95 dark:bg-slate-900/95 shadow-2xl ring-1 ring-black/5 dark:ring-white/10 backdrop-blur-2xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={t('payroll.breakdown.title')}
      >
        <div className="sticky top-0 z-10 flex items-start justify-between border-b border-gray-200 dark:border-slate-800 bg-white/90 dark:bg-slate-900/90 backdrop-blur-md px-6 py-4">
          <div>
            <h2 className="font-semibold text-gray-900 dark:text-slate-100">
              {data?.employee_name ?? t('payroll.breakdown.title')}
            </h2>
            {data && <p className="text-xs text-gray-400 dark:text-slate-500">{data.employee_number}</p>}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100 dark:text-slate-400 dark:hover:bg-slate-800 transition-colors"
            aria-label={t('payroll.breakdown.close')}
          >
            <X className="h-4 w-4" />
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
          <p className="px-6 py-8 text-sm text-red-600 dark:text-red-400">{t('payroll.breakdown.failed')}</p>
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
      </motion.div>
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
