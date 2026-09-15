import { useQuery } from '@tanstack/react-query';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getPayslipBreakdown } from '@/api/payroll';
import { formatMYR } from '@/lib/utils';
import type { PayrollItemDetail } from '@/types';

/**
 * The stored line-by-line breakdown behind one payslip.
 *
 * Payslips previously exposed only totals — `total_allowances` with no way to
 * see which allowances, `total_overtime` with no hours or rate. The engine now
 * writes these lines when the run is committed, so this shows what the run
 * actually did rather than recomputing it from inputs that may have moved.
 */
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
  const { data, isLoading, isError } = useQuery({
    queryKey: ['payslipBreakdown', runId, employeeId],
    queryFn: () => getPayslipBreakdown(runId, employeeId),
  });

  const earnings = data?.lines.filter((line) => line.category === 'earning') ?? [];
  const deductions = data?.lines.filter((line) => line.category === 'deduction') ?? [];

  return (
    <div className="fixed inset-0 z-50 flex justify-end bg-black/30" onClick={onClose}>
      <div
        className="flex h-full w-full max-w-lg flex-col overflow-y-auto bg-white shadow-xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label={t('payroll.breakdown.title')}
      >
        <div className="sticky top-0 flex items-start justify-between border-b border-gray-200 bg-white px-6 py-4">
          <div>
            <h2 className="font-semibold text-gray-900">
              {data?.employee_name ?? t('payroll.breakdown.title')}
            </h2>
            {data && <p className="text-xs text-gray-400">{data.employee_number}</p>}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100"
            aria-label={t('payroll.breakdown.close')}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {isLoading && <p className="px-6 py-8 text-sm text-gray-400">{t('payroll.breakdown.loading')}</p>}
        {isError && (
          <p className="px-6 py-8 text-sm text-red-600">{t('payroll.breakdown.failed')}</p>
        )}

        {data && (
          <div className="space-y-6 px-6 py-5">
            {data.lines.length === 0 ? (
              <p className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
                {t('payroll.breakdown.noStored')}
              </p>
            ) : (
              <>
                <LineSection title={t('payroll.breakdown.earnings')} lines={earnings} tone="text-gray-900" />
                <LineSection title={t('payroll.breakdown.deductions')} lines={deductions} tone="text-red-600" />
              </>
            )}

            <div className="space-y-2 rounded-xl border border-gray-200 bg-gray-50 p-4">
              <TotalRow label={t('payroll.breakdown.grossSalary')} value={data.item.gross_salary} />
              <TotalRow label={t('payroll.breakdown.totalDeductions')} value={data.item.total_deductions} negative />
              <div className="border-t border-gray-200 pt-2">
                <TotalRow label={t('payroll.breakdown.netSalary')} value={data.item.net_salary} emphasis />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
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
      <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500">{title}</h3>
      <div className="divide-y divide-gray-100 rounded-xl border border-gray-200">
        {lines.map((line) => (
          <div key={line.id} className="flex items-start justify-between gap-4 px-4 py-2.5">
            <div>
              <p className="text-sm text-gray-800">{line.description}</p>
              <div className="mt-0.5 flex gap-1.5">
                {line.is_statutory && (
                  <span className="rounded bg-blue-50 px-1.5 py-0.5 text-[10px] font-medium uppercase text-blue-700">
                    {t('payroll.breakdown.statutory')}
                  </span>
                )}
                {line.category === 'earning' && line.is_taxable === false && (
                  <span className="rounded bg-gray-100 px-1.5 py-0.5 text-[10px] font-medium uppercase text-gray-600">
                    {t('payroll.breakdown.nonTaxable')}
                  </span>
                )}
              </div>
            </div>
            <span className={`shrink-0 text-sm font-medium ${tone}`}>{formatMYR(line.amount)}</span>
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
      <span className={`text-sm ${emphasis ? 'font-semibold text-gray-900' : 'text-gray-600'}`}>
        {label}
      </span>
      <span
        className={`text-sm font-semibold ${
          emphasis ? 'text-green-600' : negative ? 'text-red-600' : 'text-gray-900'
        }`}
      >
        {negative ? '-' : ''}
        {formatMYR(value)}
      </span>
    </div>
  );
}
