import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Download, FileText } from 'lucide-react';
import type { ClaimsReportRow } from '@/api/reports';
import { formatMYR, formatNumber } from '@/lib/format';
import { getErrorMessage } from '@/lib/utils';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
import { SummaryCard } from './ReportShared';

const fmt = formatMYR;

export function ClaimsSummaryReport({
  data,
  isLoading,
  isError,
  error,
  onRetry,
  year,
  onExport,
}: {
  data: ClaimsReportRow[] | undefined;
  isLoading: boolean;
  isError?: boolean;
  error?: unknown;
  onRetry?: () => void;
  year: number;
  onExport: () => void;
}) {
  const { t } = useTranslation();
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!data) return [];
    if (!search.trim()) return data;
    const q = search.toLowerCase();
    return data.filter(
      (r) =>
        r.employee_name.toLowerCase().includes(q) ||
        r.employee_number.toLowerCase().includes(q) ||
        (r.department || '').toLowerCase().includes(q),
    );
  }, [data, search]);

  if (isLoading) {
    return (
      <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-4">
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-18 w-full rounded-2xl" />
          ))}
        </div>
        <Skeleton className="h-64 w-full rounded-2xl" />
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

  // Summary totals
  const totals = (data ?? []).reduce(
    (acc, r) => ({
      claims: acc.claims + r.total_claims,
      amount: acc.amount + r.total_amount,
      approved: acc.approved + r.approved_count,
      approvedAmt: acc.approvedAmt + r.approved_amount,
      pending: acc.pending + r.pending_count,
      pendingAmt: acc.pendingAmt + r.pending_amount,
      rejected: acc.rejected + r.rejected_count,
    }),
    { claims: 0, amount: 0, approved: 0, approvedAmt: 0, pending: 0, pendingAmt: 0, rejected: 0 },
  );

  const hasRawData = Boolean(data && data.length > 0);

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('reports.claimsTitle', { year })}</h2>
        <div className="no-print flex flex-wrap items-center gap-2">
          <input
            type="text"
            placeholder={t('reports.searchEmployee')}
            aria-label={t('reports.searchEmployee')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="form-input min-h-11 sm:min-h-10 sm:w-56 sm:text-sm"
          />
          {hasRawData && (
            <button type="button" onClick={onExport} className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 !text-xs">
              <Download className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.export')}
            </button>
          )}
        </div>
      </div>

      {/* Summary cards */}
      {hasRawData && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <SummaryCard label={t('reports.totalClaims')} value={formatNumber(totals.claims)} />
          <SummaryCard label={t('reports.totalAmount')} value={fmt(totals.amount)} />
          <SummaryCard
            label={t('enums.requestStatus.approved')}
            value={formatNumber(totals.approved)}
            sub={fmt(totals.approvedAmt)}
            className="text-emerald-600 dark:text-emerald-400"
          />
          <SummaryCard
            label={t('enums.requestStatus.pending')}
            value={formatNumber(totals.pending)}
            sub={fmt(totals.pendingAmt)}
            className="text-amber-600 dark:text-amber-400"
          />
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="card">
          <EmptyState
            icon={FileText}
            title={
              hasRawData && search.trim()
                ? t('reports.noMatchingEmployees')
                : t('reports.noClaimsData', { year })
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
            {filtered.map((r) => (
              <div
                key={r.employee_number}
                className="card !p-4 space-y-3"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="font-semibold text-gray-900 dark:text-slate-100 truncate">{r.employee_name}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400">
                      {r.employee_number} · {r.department || t('common.unassigned')}
                    </p>
                  </div>
                  <div className="text-right shrink-0">
                    <p className="text-sm font-semibold tabular-nums text-gray-900 dark:text-slate-100">
                      {fmt(r.total_amount)}
                    </p>
                    <p className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                      {t('reports.claimsCount', { count: r.total_claims })}
                    </p>
                  </div>
                </div>
                <div className="grid grid-cols-3 gap-2 pt-2 border-t border-gray-100 dark:border-slate-800 text-xs">
                  <div>
                    <span className="text-gray-500 dark:text-slate-400 block">{t('enums.requestStatus.approved')}</span>
                    <span className="font-semibold text-emerald-600 dark:text-emerald-400 tabular-nums">
                      {formatNumber(r.approved_count)}
                    </span>
                    {r.approved_amount > 0 && (
                      <span className="block text-emerald-600/80 dark:text-emerald-400/80 tabular-nums">
                        {fmt(r.approved_amount)}
                      </span>
                    )}
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-slate-400 block">{t('enums.requestStatus.pending')}</span>
                    <span className="font-semibold text-amber-600 dark:text-amber-400 tabular-nums">
                      {formatNumber(r.pending_count)}
                    </span>
                    {r.pending_amount > 0 && (
                      <span className="block text-amber-600/80 dark:text-amber-400/80 tabular-nums">
                        {fmt(r.pending_amount)}
                      </span>
                    )}
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-slate-400 block">{t('enums.requestStatus.rejected')}</span>
                    <span className={`font-semibold tabular-nums ${r.rejected_count > 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-500 dark:text-slate-400'}`}>
                      {formatNumber(r.rejected_count)}
                    </span>
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className="hidden md:block card !p-0 overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 dark:bg-slate-950/70 text-gray-500 dark:text-slate-400 text-xs uppercase tracking-wide">
                  <th className="text-left px-5 py-3 font-medium whitespace-nowrap">{t('common.employee')}</th>
                  <th className="text-left px-5 py-3 font-medium whitespace-nowrap">{t('common.department')}</th>
                  <th className="text-center px-5 py-3 font-medium whitespace-nowrap">{t('common.total')}</th>
                  <th className="text-right px-5 py-3 font-medium whitespace-nowrap">{t('common.amount')}</th>
                  <th className="text-center px-5 py-3 font-medium whitespace-nowrap">
                    {t('enums.requestStatus.approved')}
                  </th>
                  <th className="text-center px-5 py-3 font-medium whitespace-nowrap">
                    {t('enums.requestStatus.pending')}
                  </th>
                  <th className="text-center px-5 py-3 font-medium whitespace-nowrap">
                    {t('enums.requestStatus.rejected')}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                {filtered.map((r) => (
                  <tr key={r.employee_number} className="hover:bg-gray-50/70 dark:hover:bg-slate-800/50">
                    <td className="px-5 py-3">
                      <div className="font-semibold text-gray-900 dark:text-slate-100">{r.employee_name}</div>
                      <div className="text-xs text-gray-500 dark:text-slate-400">{r.employee_number}</div>
                    </td>
                    <td className="px-5 py-3 text-gray-600 dark:text-slate-300">{r.department || '\u2014'}</td>
                    <td className="px-5 py-3 text-center font-medium text-gray-900 dark:text-slate-100 tabular-nums">
                      {formatNumber(r.total_claims)}
                    </td>
                    <td className="px-5 py-3 text-right font-semibold text-gray-900 dark:text-slate-100 tabular-nums whitespace-nowrap">
                      {fmt(r.total_amount)}
                    </td>
                    <td className="px-5 py-3 text-center tabular-nums">
                      <span className="text-emerald-600 dark:text-emerald-400 font-medium">{formatNumber(r.approved_count)}</span>
                      {r.approved_amount > 0 && (
                        <div className="text-xs text-emerald-600/80 dark:text-emerald-400/80 whitespace-nowrap">{fmt(r.approved_amount)}</div>
                      )}
                    </td>
                    <td className="px-5 py-3 text-center tabular-nums">
                      <span className="text-amber-600 dark:text-amber-400 font-medium">{formatNumber(r.pending_count)}</span>
                      {r.pending_amount > 0 && (
                        <div className="text-xs text-amber-600/80 dark:text-amber-400/80 whitespace-nowrap">{fmt(r.pending_amount)}</div>
                      )}
                    </td>
                    <td className="px-5 py-3 text-center tabular-nums">
                      <span
                        className={`font-medium ${r.rejected_count > 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-500 dark:text-slate-400'}`}
                      >
                        {formatNumber(r.rejected_count)}
                      </span>
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
