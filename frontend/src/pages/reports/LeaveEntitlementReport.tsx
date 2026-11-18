import { useMemo, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { AlertTriangle, ChevronDown, ChevronRight, Download, FileText } from 'lucide-react';
import type { LeaveReportRow } from '@/api/reports';
import { getLeaveEligibility, groupByEmployee } from './leaveReportHelpers';

export function LeaveEntitlementReport({
  data,
  isLoading,
  year,
  onExport,
}: {
  data: LeaveReportRow[] | undefined;
  isLoading: boolean;
  year: number;
  onExport: () => void;
}) {
  const { t } = useTranslation();
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
      <div className="flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900">{t('reports.leaveTitle', { year })}</h2>
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder={t('reports.searchEmployee')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="form-input !w-auto min-w-[200px] !text-sm"
          />
          {data && data.length > 0 && (
            <button onClick={onExport} className="btn-secondary !py-1.5 !px-3 !text-xs">
              <Download className="w-3.5 h-3.5" /> {t('common.export')}
            </button>
          )}
        </div>
      </div>

      {groups.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">{t('reports.noLeaveData', { year })}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {groups.map((g) => {
            const isOpen = expandedEmployee === g.employee_number;
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
                className="bg-white rounded-xl border border-gray-200 overflow-hidden"
              >
                {/* Employee header */}
                <button
                  onClick={() => setExpandedEmployee(isOpen ? null : g.employee_number)}
                  className="w-full flex items-center gap-4 px-5 py-4 hover:bg-gray-50 transition-colors text-left"
                >
                  <div className="text-gray-400">
                    {isOpen ? (
                      <ChevronDown className="w-4 h-4" />
                    ) : (
                      <ChevronRight className="w-4 h-4" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-semibold text-gray-900">{g.employee_name}</div>
                    <div className="text-xs text-gray-400">
                      {g.employee_number} · {g.department || t('reports.noDepartment')}
                    </div>
                  </div>
                  <div className="flex items-center gap-6 text-sm shrink-0">
                    <div className="text-center">
                      <div className="text-xs text-gray-400">{t('reports.taken')}</div>
                      <div className="font-medium text-gray-700">{totalTaken}d</div>
                    </div>
                    <div className="text-center">
                      <div className="text-xs text-gray-400">{t('reports.balance')}</div>
                      <div
                        className={`font-bold ${totalBalance > 0 ? 'text-emerald-600' : 'text-gray-600'}`}
                      >
                        {totalBalance}d
                      </div>
                    </div>
                  </div>
                </button>

                {/* Expanded leave table */}
                {isOpen && (
                  <div className="border-t border-gray-100">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                          <th className="text-left px-5 py-2.5 font-medium">
                            {t('approvals.leaveType')}
                          </th>
                          <th className="text-left px-5 py-2.5 font-medium">
                            {t('reports.statusRemarks')}
                          </th>
                          <th className="text-center px-5 py-2.5 font-medium">
                            {t('reports.entitled')}
                          </th>
                          <th className="text-center px-5 py-2.5 font-medium">
                            {t('reports.taken')}
                          </th>
                          <th className="text-center px-5 py-2.5 font-medium">
                            {t('enums.requestStatus.pending')}
                          </th>
                          <th className="text-center px-5 py-2.5 font-medium">
                            {t('reports.balance')}
                          </th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-gray-50">
                        {g.leaves.map((l, i) => {
                          const elig = getLeaveEligibility(
                            t,
                            l.leave_type_name,
                            g.gender,
                            g.marital_status,
                            g.num_children,
                          );
                          return (
                            <tr key={i} className={!elig.eligible ? 'bg-gray-50/50' : ''}>
                              <td className="px-5 py-3 font-medium text-gray-900">
                                {l.leave_type_name}
                              </td>
                              <td className="px-5 py-3">
                                {elig.eligible ? (
                                  <span className="text-gray-600">
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
                                  <span className="inline-flex items-center gap-1.5 text-amber-600 text-sm">
                                    <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                                    {elig.reason}
                                  </span>
                                )}
                              </td>
                              <td className="px-5 py-3 text-center text-gray-600">
                                {elig.eligible ? Number(l.entitled_days) : '\u2014'}
                              </td>
                              <td className="px-5 py-3 text-center text-gray-600">
                                {elig.eligible ? Number(l.taken_days) : '\u2014'}
                              </td>
                              <td className="px-5 py-3 text-center">
                                {elig.eligible ? (
                                  <span
                                    className={
                                      Number(l.pending_days) > 0
                                        ? 'text-amber-600'
                                        : 'text-gray-600'
                                    }
                                  >
                                    {Number(l.pending_days)}
                                  </span>
                                ) : (
                                  '\u2014'
                                )}
                              </td>
                              <td className="px-5 py-3 text-center">
                                {elig.eligible ? (
                                  <span
                                    className={`font-bold ${
                                      Number(l.balance) > 0
                                        ? 'text-emerald-600'
                                        : Number(l.balance) < 0
                                          ? 'text-red-600'
                                          : 'text-gray-600'
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
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
