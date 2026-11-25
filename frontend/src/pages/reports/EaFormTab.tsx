import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Download, FileText } from 'lucide-react';
import { downloadEaForm } from '@/api/reports';
import { formatMYR, formatNumber } from '@/lib/format';
import { getErrorMessage } from '@/lib/utils';
import { useToast } from '@/lib/toast';
import { EmptyState } from '@/components/ui/EmptyState';
import type { EaEmployeeSummary } from '@/types';

const fmt = formatMYR;

export function EaFormTab({
  data,
  isLoading,
  isError,
  error,
  onRetry,
  year,
}: {
  data: EaEmployeeSummary[] | undefined;
  isLoading: boolean;
  isError?: boolean;
  error?: unknown;
  onRetry?: () => void;
  year: number;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(() => new Set());

  const handleDownload = async (employeeId: string) => {
    setDownloadingIds((prev) => {
      const next = new Set(prev);
      next.add(employeeId);
      return next;
    });
    try {
      await downloadEaForm(year, employeeId);
    } catch (err) {
      toast.error(getErrorMessage(err, t('reports.downloadFailed')));
    } finally {
      setDownloadingIds((prev) => {
        const next = new Set(prev);
        next.delete(employeeId);
        return next;
      });
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-48" role="status">
        <span className="sr-only">{t('common.loading')}</span>
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--accent-1)]" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="card">
        <EmptyState
          icon={AlertTriangle}
          title={t('reports.loadFailed')}
          description={getErrorMessage(error, t('reports.loadFailed'))}
          action={
            onRetry ? (
              <button type="button" onClick={onRetry} className="btn-primary">
                {t('common.retry')}
              </button>
            ) : undefined
          }
        />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('reports.eaTitle', { year })}</h2>
      {!data || data.length === 0 ? (
        <div className="text-center py-16 text-gray-500 dark:text-slate-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" aria-hidden="true" />
          <p className="text-sm">{t('reports.noPayrollData', { year })}</p>
        </div>
      ) : (
        <>
          <div className="space-y-3 md:hidden">
            {data.map((emp) => (
              <div
                key={emp.employee_id}
                className="bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 p-4 space-y-3"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="font-semibold text-gray-900 dark:text-slate-100 truncate">{emp.employee_name}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400">{emp.employee_number}</p>
                  </div>
                  <p className="text-sm font-semibold tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
                    {fmt(emp.ytd_gross)}
                  </p>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs text-gray-600 dark:text-slate-300">
                  <div>
                    <span className="text-gray-400 dark:text-slate-500 block">{t('reports.cols.icNumber')}</span>
                    <span className="tabular-nums">{emp.ic_number || '\u2014'}</span>
                  </div>
                  <div>
                    <span className="text-gray-400 dark:text-slate-500 block">{t('reports.cols.months')}</span>
                    <span className="tabular-nums">{formatNumber(emp.months_worked)}</span>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => handleDownload(emp.employee_id)}
                  disabled={downloadingIds.has(emp.employee_id)}
                  className="no-print btn-secondary w-full min-h-11 justify-center text-xs disabled:opacity-50"
                >
                  <Download className="w-3.5 h-3.5" aria-hidden="true" />
                  {downloadingIds.has(emp.employee_id) ? t('common.generating') : t('reports.downloadPdf')}
                </button>
              </div>
            ))}
          </div>

          <div className="hidden md:block bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 dark:bg-slate-950/70 text-gray-500 dark:text-slate-400 text-xs uppercase tracking-wide">
                  <th className="text-left px-5 py-3 font-medium whitespace-nowrap">{t('common.employee')}</th>
                  <th className="text-left px-5 py-3 font-medium whitespace-nowrap">{t('reports.cols.icNumber')}</th>
                  <th className="text-center px-5 py-3 font-medium whitespace-nowrap">{t('reports.cols.months')}</th>
                  <th className="text-right px-5 py-3 font-medium whitespace-nowrap">{t('reports.cols.ytdGross')}</th>
                  <th className="no-print text-right px-5 py-3 font-medium whitespace-nowrap">{t('common.action')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                {data.map((emp) => (
                  <tr key={emp.employee_id} className="hover:bg-gray-50/70 dark:hover:bg-slate-800/50">
                    <td className="px-5 py-3">
                      <div className="font-semibold text-gray-900 dark:text-slate-100">{emp.employee_name}</div>
                      <div className="text-xs text-gray-500 dark:text-slate-400">{emp.employee_number}</div>
                    </td>
                    <td className="px-5 py-3 text-gray-600 dark:text-slate-300 tabular-nums whitespace-nowrap">
                      {emp.ic_number || '\u2014'}
                    </td>
                    <td className="px-5 py-3 text-center text-gray-700 dark:text-slate-300 tabular-nums">
                      {formatNumber(emp.months_worked)}
                    </td>
                    <td className="px-5 py-3 text-right font-medium text-gray-900 dark:text-slate-100 tabular-nums whitespace-nowrap">
                      {fmt(emp.ytd_gross)}
                    </td>
                    <td className="no-print px-5 py-3 text-right">
                      <button
                        type="button"
                        onClick={() => handleDownload(emp.employee_id)}
                        disabled={downloadingIds.has(emp.employee_id)}
                        className="btn-secondary min-h-9 !py-1.5 !px-3 !text-xs disabled:opacity-50"
                      >
                        <Download className="w-3.5 h-3.5" aria-hidden="true" />
                        {downloadingIds.has(emp.employee_id)
                          ? t('common.generating')
                          : t('reports.downloadPdf')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
