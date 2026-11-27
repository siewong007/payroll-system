import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Download, FileText, Loader2 } from 'lucide-react';
import { downloadEaForm } from '@/api/reports';
import { formatMYR, formatNumber } from '@/lib/format';
import { getErrorMessage } from '@/lib/utils';
import { useToast } from '@/lib/toast';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
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
  const [search, setSearch] = useState('');
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(() => new Set());

  const filtered = useMemo(() => {
    if (!data) return [];
    if (!search.trim()) return data;
    const q = search.toLowerCase();
    return data.filter(
      (emp) =>
        emp.employee_name.toLowerCase().includes(q) ||
        emp.employee_number.toLowerCase().includes(q) ||
        (emp.ic_number || '').toLowerCase().includes(q),
    );
  }, [data, search]);

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
      <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-3">
        {[0, 1, 2, 3].map((i) => (
          <Skeleton key={i} className="h-16 w-full rounded-xl" />
        ))}
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

  const hasRawData = Boolean(data && data.length > 0);

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('reports.eaTitle', { year })}</h2>
        {hasRawData && (
          <input
            type="text"
            placeholder={t('reports.searchEmployee')}
            aria-label={t('reports.searchEmployee')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="no-print form-input min-h-11 sm:min-h-10 sm:w-56 sm:text-sm"
          />
        )}
      </div>
      {filtered.length === 0 ? (
        <div className="card">
          <EmptyState
            icon={FileText}
            title={
              hasRawData && search.trim()
                ? t('reports.noMatchingEmployees')
                : t('reports.noPayrollData', { year })
            }
            action={
              hasRawData && search.trim() ? (
                <button type="button" onClick={() => setSearch('')} className="btn-secondary min-h-11 sm:min-h-9">
                  {t('reports.clearSearch')}
                </button>
              ) : undefined
            }
          />
        </div>
      ) : (
        <>
          <div className="space-y-3 md:hidden">
            {filtered.map((emp) => {
              const isDownloading = downloadingIds.has(emp.employee_id);
              const downloadLabel = `${t('reports.downloadPdf')} \u2014 ${emp.employee_name}`;
              return (
                <div
                  key={emp.employee_id}
                  className="card !p-4 space-y-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="font-semibold text-gray-900 dark:text-slate-100 truncate">{emp.employee_name}</p>
                      <p className="text-xs text-gray-500 dark:text-slate-400">{emp.employee_number}</p>
                    </div>
                    <div className="text-right shrink-0">
                      <span className="text-xs text-gray-500 dark:text-slate-400 block">{t('reports.cols.ytdGross')}</span>
                      <p className="text-sm font-semibold tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
                        {fmt(emp.ytd_gross)}
                      </p>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-2 text-xs text-gray-600 dark:text-slate-300">
                    <div>
                      <span className="text-gray-500 dark:text-slate-400 block">{t('reports.cols.icNumber')}</span>
                      <span className="tabular-nums">{emp.ic_number || '\u2014'}</span>
                    </div>
                    <div>
                      <span className="text-gray-500 dark:text-slate-400 block">{t('reports.cols.months')}</span>
                      <span className="tabular-nums">{formatNumber(emp.months_worked)}</span>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => handleDownload(emp.employee_id)}
                    disabled={isDownloading}
                    aria-label={downloadLabel}
                    className="no-print btn-secondary w-full min-h-11 justify-center text-xs disabled:opacity-50"
                  >
                    {isDownloading ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <Download className="w-3.5 h-3.5" aria-hidden="true" />
                    )}
                    {isDownloading ? t('common.generating') : t('reports.downloadPdf')}
                  </button>
                </div>
              );
            })}
          </div>

          <div className="hidden md:block card !p-0 overflow-x-auto">
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
                {filtered.map((emp) => {
                  const isDownloading = downloadingIds.has(emp.employee_id);
                  const downloadLabel = `${t('reports.downloadPdf')} \u2014 ${emp.employee_name}`;
                  return (
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
                          disabled={isDownloading}
                          aria-label={downloadLabel}
                          className="btn-secondary min-h-9 !py-1.5 !px-3 !text-xs disabled:opacity-50"
                        >
                          {isDownloading ? (
                            <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                          ) : (
                            <Download className="w-3.5 h-3.5" aria-hidden="true" />
                          )}
                          {isDownloading ? t('common.generating') : t('reports.downloadPdf')}
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
