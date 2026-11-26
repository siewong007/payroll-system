import { useId, useMemo, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { AlertTriangle, ChevronDown, ChevronRight, Download, FileText } from 'lucide-react';
import type { LeaveReportRow } from '@/api/reports';
import { getErrorMessage } from '@/lib/utils';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
import { getLeaveEligibility, groupByEmployee } from './leaveReportHelpers';

function formatDayValue(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(2)));
}

export function LeaveEntitlementReport({
  data,
  isLoading,
  isError,
  error,
  onRetry,
  year,
  onExport,
}: {
  data: LeaveReportRow[] | undefined;
  isLoading: boolean;
  isError?: boolean;
  error?: unknown;
  onRetry?: () => void;
  year: number;
  onExport: () => void;
}) {
  const { t } = useTranslation();
  const panelPrefix = useId();
  const [expandedEmployee, setExpandedEmployee] = useState<string | null>(null);
  const [search, setSearch] = useState('');

  const groups = useMemo(() => {
    if (!data) return [];
    const all = groupByEmployee(data);
    if (!search.trim()) return all;
    const q = search.toLowerCase();
    return all.filter(
      (g) =>
        g.employee_name.toLowerCase().includes(q) ||
        g.employee_number.toLowerCase().includes(q) ||
        (g.department || '').toLowerCase().includes(q),
    );
  }, [data, search]);

  if (isLoading) {
    return (
      <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-2.5">
        {[0, 1, 2, 3].map((i) => (
          <Skeleton key={i} className="h-18 w-full rounded-2xl" />
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
        <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('reports.leaveTitle', { year })}</h2>
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

      {groups.length === 0 ? (
        <div className="card">
          <EmptyState
            icon={FileText}
            title={
              hasRawData && search.trim()
                ? t('reports.noMatchingEmployees')
                : t('reports.noLeaveData', { year })
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
        <div className="space-y-2">
          {groups.map((g) => {
            const isOpen = expandedEmployee === g.employee_number;
            const panelId = `${panelPrefix}-${g.employee_number}`;
            const totalBalance = g.leaves.reduce((sum, l) => {
              const elig = getLeaveEligibility(
                t,
                l.leave_type_name,
                g.gender,
                g.marital_status,
                g.num_children,
              );
              return elig.eligible ? sum + Number(l.balance) : sum;
            }, 0);
            const totalTaken = g.leaves.reduce((sum, l) => {
              const elig = getLeaveEligibility(
                t,
                l.leave_type_name,
                g.gender,
                g.marital_status,
                g.num_children,
              );
              return elig.eligible ? sum + Number(l.taken_days) : sum;
            }, 0);

            return (
              <div
                key={g.employee_number}
                className="card !p-0 overflow-hidden"
              >
                {/* Employee header */}
                <button
                  type="button"
                  aria-expanded={isOpen}
                  aria-controls={panelId}
                  onClick={() => setExpandedEmployee(isOpen ? null : g.employee_number)}
                  className="w-full min-h-11 flex items-center gap-3 sm:gap-4 px-4 sm:px-5 py-4 hover:bg-gray-50 dark:hover:bg-slate-800/50 transition-colors text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--ring)]"
                >
                  <div className="text-gray-500 dark:text-slate-400 shrink-0" aria-hidden="true">
                    {isOpen ? (
                      <ChevronDown className="w-4 h-4" />
                    ) : (
                      <ChevronRight className="w-4 h-4" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-semibold text-gray-900 dark:text-slate-100 truncate">{g.employee_name}</div>
                    <div className="text-xs text-gray-500 dark:text-slate-400 truncate">
                      {g.employee_number} · {g.department || t('reports.noDepartment')}
                    </div>
                  </div>
                  <div className="flex items-center gap-4 sm:gap-6 text-sm shrink-0">
                    <div className="text-center">
                      <div className="text-xs text-gray-500 dark:text-slate-400">{t('reports.taken')}</div>
                      <div className="font-medium text-gray-700 dark:text-slate-200 tabular-nums">
                        {t('reports.daysShort', { days: formatDayValue(totalTaken) })}
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="text-xs text-gray-500 dark:text-slate-400">{t('reports.balance')}</div>
                      <div
                        className={`font-bold tabular-nums ${totalBalance > 0 ? 'text-emerald-600 dark:text-emerald-400' : 'text-gray-600 dark:text-slate-300'}`}
                      >
                        {t('reports.daysShort', { days: formatDayValue(totalBalance) })}
                      </div>
                    </div>
                  </div>
                </button>

                {/* Expanded leave details */}
                {isOpen && (
                  <div id={panelId} className="border-t border-gray-100 dark:border-slate-800">
                    <div className="divide-y divide-gray-100 dark:divide-slate-800 md:hidden">
                      {g.leaves.map((l) => {
                        const elig = getLeaveEligibility(
                          t,
                          l.leave_type_name,
                          g.gender,
                          g.marital_status,
                          g.num_children,
                        );
                        return (
                          <div key={l.leave_type_name} className={`p-4 space-y-2 ${!elig.eligible ? 'bg-gray-50/70 dark:bg-slate-800/30' : ''}`}>
                            <p className="font-medium text-sm text-gray-900 dark:text-slate-100">{l.leave_type_name}</p>
                            <div>
                              {elig.eligible ? (
                                <span className="text-xs text-gray-600 dark:text-slate-300">
                                  <Trans
                                    i18nKey="reports.hasDays"
                                    values={{
                                      count: Number(l.balance),
                                      type: l.leave_type_name,
                                    }}
                                    components={{ strong: <strong /> }}
                                  />
                                </span>
                              ) : (
                                <span className="inline-flex items-center gap-1.5 text-amber-600 dark:text-amber-400 text-xs">
                                  <AlertTriangle className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                                  {elig.reason}
                                </span>
                              )}
                            </div>
                            {elig.eligible && (
                              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 pt-1 text-xs tabular-nums">
                                <div>
                                  <span className="text-gray-500 dark:text-slate-400 block">{t('reports.entitled')}</span>
                                  <span className="text-gray-700 dark:text-slate-200">{Number(l.entitled_days)}</span>
                                </div>
                                <div>
                                  <span className="text-gray-500 dark:text-slate-400 block">{t('reports.taken')}</span>
                                  <span className="text-gray-700 dark:text-slate-200">{Number(l.taken_days)}</span>
                                </div>
                                <div>
                                  <span className="text-gray-500 dark:text-slate-400 block">{t('enums.requestStatus.pending')}</span>
                                  <span className={Number(l.pending_days) > 0 ? 'text-amber-600 dark:text-amber-400' : 'text-gray-700 dark:text-slate-200'}>
                                    {Number(l.pending_days)}
                                  </span>
                                </div>
                                <div>
                                  <span className="text-gray-500 dark:text-slate-400 block">{t('reports.balance')}</span>
                                  <span className={`font-bold ${Number(l.balance) > 0 ? 'text-emerald-600 dark:text-emerald-400' : Number(l.balance) < 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-700 dark:text-slate-200'}`}>
                                    {Number(l.balance)}
                                  </span>
                                </div>
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>

                    <div className="hidden md:block overflow-x-auto">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="bg-gray-50 dark:bg-slate-950/70 text-gray-500 dark:text-slate-400 text-xs uppercase tracking-wide">
                            <th className="text-left px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('approvals.leaveType')}
                            </th>
                            <th className="text-left px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('reports.statusRemarks')}
                            </th>
                            <th className="text-center px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('reports.entitled')}
                            </th>
                            <th className="text-center px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('reports.taken')}
                            </th>
                            <th className="text-center px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('enums.requestStatus.pending')}
                            </th>
                            <th className="text-center px-5 py-2.5 font-medium whitespace-nowrap">
                              {t('reports.balance')}
                            </th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                          {g.leaves.map((l) => {
                            const elig = getLeaveEligibility(
                              t,
                              l.leave_type_name,
                              g.gender,
                              g.marital_status,
                              g.num_children,
                            );
                            return (
                              <tr key={l.leave_type_name} className={!elig.eligible ? 'bg-gray-50/70 dark:bg-slate-800/30' : ''}>
                                <td className="px-5 py-3 font-medium text-gray-900 dark:text-slate-100 whitespace-nowrap">
                                  {l.leave_type_name}
                                </td>
                                <td className="px-5 py-3">
                                  {elig.eligible ? (
                                    <span className="text-gray-700 dark:text-slate-300">
                                      <Trans
                                        i18nKey="reports.hasDays"
                                        values={{
                                          count: Number(l.balance),
                                          type: l.leave_type_name,
                                        }}
                                        components={{ strong: <strong /> }}
                                      />
                                    </span>
                                  ) : (
                                    <span className="inline-flex items-center gap-1.5 text-amber-600 dark:text-amber-400 text-sm">
                                      <AlertTriangle className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                                      {elig.reason}
                                    </span>
                                  )}
                                </td>
                                <td className="px-5 py-3 text-center text-gray-700 dark:text-slate-300 tabular-nums">
                                  {elig.eligible ? Number(l.entitled_days) : '\u2014'}
                                </td>
                                <td className="px-5 py-3 text-center text-gray-700 dark:text-slate-300 tabular-nums">
                                  {elig.eligible ? Number(l.taken_days) : '\u2014'}
                                </td>
                                <td className="px-5 py-3 text-center tabular-nums">
                                  {elig.eligible ? (
                                    <span
                                      className={
                                        Number(l.pending_days) > 0
                                          ? 'text-amber-600 dark:text-amber-400'
                                          : 'text-gray-700 dark:text-slate-300'
                                      }
                                    >
                                      {Number(l.pending_days)}
                                    </span>
                                  ) : (
                                    '\u2014'
                                  )}
                                </td>
                                <td className="px-5 py-3 text-center tabular-nums">
                                  {elig.eligible ? (
                                    <span
                                      className={`font-bold ${
                                        Number(l.balance) > 0
                                          ? 'text-emerald-600 dark:text-emerald-400'
                                          : Number(l.balance) < 0
                                            ? 'text-red-600 dark:text-red-400'
                                            : 'text-gray-700 dark:text-slate-300'
                                      }`}
                                    >
                                      {Number(l.balance)}
                                    </span>
                                  ) : (
                                    '\u2014'
                                  )}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
