import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, FileText } from 'lucide-react';
import type { ClaimsReportRow } from '@/api/reports';
import { formatMYR } from '@/lib/format';
import { SummaryCard } from './ReportShared';

const fmt = formatMYR;

export function ClaimsSummaryReport({
  data,
  isLoading,
  year,
  onExport,
}: {
  data: ClaimsReportRow[] | undefined;
  isLoading: boolean;
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
      <div className="flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
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

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900">{t('reports.claimsTitle', { year })}</h2>
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

      {/* Summary cards */}
      {data && data.length > 0 && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <SummaryCard label={t('reports.totalClaims')} value={String(totals.claims)} />
          <SummaryCard label={t('reports.totalAmount')} value={fmt(totals.amount)} />
          <SummaryCard
            label={t('enums.requestStatus.approved')}
            value={`${totals.approved} (${fmt(totals.approvedAmt)})`}
            className="text-emerald-600"
          />
          <SummaryCard
            label={t('enums.requestStatus.pending')}
            value={`${totals.pending} (${fmt(totals.pendingAmt)})`}
            className="text-amber-600"
          />
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">{t('reports.noClaimsData', { year })}</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                <th className="text-left px-5 py-3 font-medium">{t('common.employee')}</th>
                <th className="text-left px-5 py-3 font-medium">{t('common.department')}</th>
                <th className="text-center px-5 py-3 font-medium">{t('common.total')}</th>
                <th className="text-right px-5 py-3 font-medium">{t('common.amount')}</th>
                <th className="text-center px-5 py-3 font-medium">
                  {t('enums.requestStatus.approved')}
                </th>
                <th className="text-center px-5 py-3 font-medium">
                  {t('enums.requestStatus.pending')}
                </th>
                <th className="text-center px-5 py-3 font-medium">
                  {t('enums.requestStatus.rejected')}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-50">
              {filtered.map((r) => (
                <tr key={r.employee_number} className="hover:bg-gray-50/50">
                  <td className="px-5 py-3">
                    <div className="font-semibold text-gray-900">{r.employee_name}</div>
                    <div className="text-xs text-gray-400">{r.employee_number}</div>
                  </td>
                  <td className="px-5 py-3 text-gray-500">{r.department || '\u2014'}</td>
                  <td className="px-5 py-3 text-center font-medium text-gray-900">
                    {r.total_claims}
                  </td>
                  <td className="px-5 py-3 text-right font-semibold text-gray-900">
                    {fmt(r.total_amount)}
                  </td>
                  <td className="px-5 py-3 text-center">
                    <span className="text-emerald-600 font-medium">{r.approved_count}</span>
                    {r.approved_amount > 0 && (
                      <div className="text-xs text-emerald-500">{fmt(r.approved_amount)}</div>
                    )}
                  </td>
                  <td className="px-5 py-3 text-center">
                    <span className="text-amber-600 font-medium">{r.pending_count}</span>
                    {r.pending_amount > 0 && (
                      <div className="text-xs text-amber-500">{fmt(r.pending_amount)}</div>
                    )}
                  </td>
                  <td className="px-5 py-3 text-center">
                    <span
                      className={`font-medium ${r.rejected_count > 0 ? 'text-red-600' : 'text-gray-400'}`}
                    >
                      {r.rejected_count}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
