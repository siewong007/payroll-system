import { AlertTriangle, CheckCircle2, Info, XCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { formatMYR } from '@/lib/utils';
import { formatDate, formatNumber } from '@/lib/format';
import { payrollDiagnosticText } from './statusMeta';
import { translateServerMessage } from '@/lib/utils';
import type { PayrollDiagnostic, PayrollPreview } from '@/types';

/**
 * Review step for a payroll run: who is included, what they would be paid, and
 * everything wrong with the run — before anything is written.
 *
 * Processing used to be a single button with no way to see the outcome first,
 * and a failure reported only the first employee that could not be calculated.
 */
export function PayrollPreviewPanel({
  preview,
  onProcess,
  onBack,
  isProcessing,
}: {
  preview: PayrollPreview;
  onProcess: () => void;
  onBack: () => void;
  isProcessing: boolean;
}) {
  const { t } = useTranslation();
  const failedCount = preview.employee_count - preview.payable_count;

  return (
    <div className="space-y-6">
      <div
        className={`rounded-2xl border p-4 ${
          preview.can_process
            ? 'border-green-200 bg-green-50'
            : 'border-red-200 bg-red-50'
        }`}
      >
        <div className="flex items-start gap-3">
          {preview.can_process ? (
            <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-green-600" />
          ) : (
            <XCircle className="mt-0.5 h-5 w-5 shrink-0 text-red-600" />
          )}
          <div>
            <p className={`font-medium ${preview.can_process ? 'text-green-800' : 'text-red-800'}`}>
              {preview.can_process
                ? t('payroll.preview.ready', { count: preview.payable_count })
                : t('payroll.preview.cannotProcess')}
            </p>
            <p className={`text-sm ${preview.can_process ? 'text-green-700' : 'text-red-700'}`}>
              {t('payroll.preview.range', {
                start: formatDate(preview.period_start),
                end: formatDate(preview.period_end),
                payDate: formatDate(preview.pay_date),
              })}
              {failedCount > 0 &&
                ` ${t('payroll.preview.failedCount', { count: failedCount, total: preview.employee_count })}`}
            </p>
          </div>
        </div>
      </div>

      <DiagnosticList
        title={t('payroll.preview.blockingTitle')}
        diagnostics={preview.blocking}
        tone="error"
      />
      <DiagnosticList title={t('payroll.preview.warningsTitle')} diagnostics={preview.warnings} tone="warning" />

      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        <TotalCard label={t('payroll.preview.employees')} value={formatNumber(preview.employee_count)} />
        <TotalCard label={t('payroll.totals.gross')} value={formatMYR(preview.total_gross)} />
        <TotalCard label={t('payroll.totals.net')} value={formatMYR(preview.total_net)} emphasis />
        <TotalCard label={t('payroll.totals.employerCost')} value={formatMYR(preview.total_employer_cost)} />
        <TotalCard
          label={t('payroll.preview.epfSplit')}
          value={`${formatMYR(preview.total_epf_employee)} + ${formatMYR(preview.total_epf_employer)}`}
          small
        />
        <TotalCard
          label={t('payroll.preview.socsoSplit')}
          value={`${formatMYR(preview.total_socso_employee)} + ${formatMYR(preview.total_socso_employer)}`}
          small
        />
        <TotalCard
          label={t('payroll.preview.eisSplit')}
          value={`${formatMYR(preview.total_eis_employee)} + ${formatMYR(preview.total_eis_employer)}`}
          small
        />
        <TotalCard label="PCB" value={formatMYR(preview.total_pcb)} small />
      </div>

      <div className="overflow-hidden rounded-2xl border border-gray-200 bg-white shadow">
        <div className="border-b border-gray-100 px-6 py-4">
          <h2 className="font-semibold text-gray-900">{t('payroll.preview.projectedTitle')}</h2>
          <p className="text-sm text-gray-500">
            {t('payroll.preview.projectedBody')}
          </p>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 text-left">
              <tr>
                <th className="px-4 py-3 text-xs font-medium uppercase tracking-wide text-gray-500">{t('employees.columns.employee')}</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">{t('payroll.cols.basic')}</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">{t('payroll.cols.allowances')}</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">{t('payroll.cols.ot')}</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">{t('payroll.cols.gross')}</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">EPF</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">SOCSO</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">EIS</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">PCB</th>
                <th className="px-4 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">{t('payroll.cols.net')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {preview.employees.map((employee) => (
                <tr
                  key={employee.employee_id}
                  className={employee.error ? 'bg-red-50/60' : 'hover:bg-gray-50'}
                >
                  <td className="px-4 py-3">
                    <div className="text-sm font-medium text-gray-900">{employee.employee_name}</div>
                    <div className="text-xs text-gray-400">{employee.employee_number}</div>
                    {employee.is_prorated && !employee.error && (
                      <div className="mt-1 inline-flex rounded-full bg-amber-50 px-2 py-0.5 text-xs text-amber-700">
                        {t('payroll.preview.prorated', { worked: employee.days_worked, total: employee.working_days })}
                      </div>
                    )}
                    {employee.error && (
                      <div className="mt-1 text-xs text-red-700">{translateServerMessage(employee.error)}</div>
                    )}
                  </td>
                  {employee.error ? (
                    <td colSpan={9} className="px-4 py-3 text-right text-sm text-red-600">
                      {t('payroll.preview.notCalculable')}
                    </td>
                  ) : (
                    <>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.basic_salary)}</td>
                      <td className="px-4 py-3 text-right text-sm">
                        {employee.total_allowances > 0 ? formatMYR(employee.total_allowances) : '-'}
                      </td>
                      <td className="px-4 py-3 text-right text-sm">
                        {employee.total_overtime > 0 ? formatMYR(employee.total_overtime) : '-'}
                      </td>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.gross_salary)}</td>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.epf_employee)}</td>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.socso_employee)}</td>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.eis_employee)}</td>
                      <td className="px-4 py-3 text-right text-sm">{formatMYR(employee.pcb_amount)}</td>
                      <td className="px-4 py-3 text-right text-sm font-bold text-green-600">
                        {formatMYR(employee.net_salary)}
                      </td>
                    </>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="flex flex-col gap-3 sm:flex-row">
        <button
          type="button"
          onClick={onProcess}
          disabled={!preview.can_process || isProcessing}
          className="flex items-center justify-center gap-2 rounded-lg bg-black px-6 py-2.5 font-medium text-white transition-colors hover:bg-gray-800 disabled:opacity-50"
        >
          <CheckCircle2 className="h-4 w-4" />
          {isProcessing ? t('payroll.preview.processing') : t('payroll.preview.confirmButton')}
        </button>
        <button
          type="button"
          onClick={onBack}
          disabled={isProcessing}
          className="rounded-lg border border-gray-200 px-6 py-2.5 font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:opacity-50"
        >
          {t('payroll.preview.backToAdjustments')}
        </button>
      </div>
    </div>
  );
}

function DiagnosticList({
  title,
  diagnostics,
  tone,
}: {
  title: string;
  diagnostics: PayrollDiagnostic[];
  tone: 'error' | 'warning';
}) {
  if (diagnostics.length === 0) return null;

  const styles =
    tone === 'error'
      ? { box: 'border-red-200 bg-red-50', heading: 'text-red-800', body: 'text-red-700' }
      : { box: 'border-amber-200 bg-amber-50', heading: 'text-amber-800', body: 'text-amber-700' };
  const Icon = tone === 'error' ? AlertTriangle : Info;

  return (
    <div className={`rounded-2xl border p-4 ${styles.box}`}>
      <div className={`mb-2 flex items-center gap-2 font-medium ${styles.heading}`}>
        <Icon className="h-4 w-4" />
        {title} ({diagnostics.length})
      </div>
      <ul className={`space-y-1.5 text-sm ${styles.body}`}>
        {diagnostics.map((diagnostic, index) => (
          <li key={`${diagnostic.code}-${diagnostic.employee_id ?? index}`} className="flex gap-2">
            <span aria-hidden="true">•</span>
            <span>
              {diagnostic.employee_number && (
                <span className="font-medium">
                  {diagnostic.employee_number} {diagnostic.employee_name}:{' '}
                </span>
              )}
              {payrollDiagnosticText(diagnostic)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function TotalCard({
  label,
  value,
  emphasis,
  small,
}: {
  label: string;
  value: string;
  emphasis?: boolean;
  small?: boolean;
}) {
  return (
    <div className="rounded-2xl border border-gray-200 bg-white p-4 shadow">
      <p className="text-xs text-gray-400">{label}</p>
      <p
        className={`mt-1 font-bold ${small ? 'text-sm' : 'text-lg'} ${
          emphasis ? 'text-green-600' : 'text-gray-900'
        }`}
      >
        {value}
      </p>
    </div>
  );
}
