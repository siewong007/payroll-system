import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import {
  AlertCircle,
  AlertTriangle,
  Calendar,
  CheckCircle2,
  Clock,
  Download,
  LogOut,
  Timer,
} from 'lucide-react';
import {
  downloadAttendanceCsv,
  getAttendanceSummary,
  type AttendanceSummaryItem,
} from '@/api/attendance';
import { getErrorMessage } from '@/lib/utils';
import { attStatusLabel, firstOfMonth } from './attendanceHelpers';

export function StatsBar({ today, onShowOpen }: { today: string; onShowOpen: () => void }) {
  const { t } = useTranslation();
  // These tiles report today across the whole company, so they must not be
  // derived from `records` — that is one 50-row page of the *currently filtered*
  // query, so a date filter or a second page made the numbers undercount or
  // collapse to zero while still being labelled "today".
  // Key is prefixed with 'attendance-records' so the existing invalidation after
  // a manual entry or edit refreshes these tiles too.
  const { data, isError } = useQuery({
    queryKey: ['attendance-records', 'today-stats', today],
    queryFn: () => getAttendanceSummary({ date_from: today, date_to: today }),
  });
  const items = data ?? [];
  const present = items.reduce((sum, i) => sum + i.present_days, 0);
  const late = items.reduce((sum, i) => sum + i.late_days, 0);
  const stillIn = items.reduce((sum, i) => sum + i.unchecked_out_days, 0);

  if (isError) {
    return (
      <div className="bg-white rounded-2xl shadow p-5 mb-6 flex items-center gap-3 text-sm text-gray-600">
        <AlertCircle className="w-5 h-5 text-red-400 shrink-0" />
        {t('attendance.stats.loadFailed')}
      </div>
    );
  }

  const tiles = [
    {
      icon: CheckCircle2,
      label: t('attendance.stats.presentToday'),
      short: t('attendance.stats.present'),
      value: present,
      color: 'text-emerald-600',
      bg: 'bg-emerald-50',
      action: undefined,
    },
    {
      icon: Clock,
      label: t('attendance.stats.late'),
      short: t('attendance.stats.late'),
      value: late,
      color: 'text-amber-600',
      bg: 'bg-amber-50',
      action: undefined,
    },
    // Actionable: these are the sessions someone has to chase or correct.
    {
      icon: LogOut,
      label: t('attendance.stats.stillIn'),
      short: t('attendance.stats.stillInShort'),
      value: stillIn,
      color: 'text-blue-600',
      bg: 'bg-blue-50',
      action: onShowOpen,
    },
  ];

  return (
    <div className="grid grid-cols-3 gap-2.5 sm:gap-4 mb-6">
      {tiles.map((s) => {
        // Phone tiles stack icon over value: three side-by-side rows at 375px
        // pushed the label clean off the right edge of the card.
        const body = (
          <>
            <div className={`${s.bg} ${s.color} p-2 sm:p-3 rounded-xl shrink-0 w-fit`}>
              <s.icon className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
            <div className="min-w-0">
              <p className="text-xl sm:text-2xl font-bold text-gray-900 leading-tight">{s.value}</p>
              <p className="text-[11px] sm:text-xs text-gray-500 leading-tight">
                <span className="sm:hidden">{s.short}</span>
                <span className="hidden sm:inline">{s.label}</span>
                {s.action && s.value > 0 && (
                  <span className="ml-1 text-blue-600 font-medium">
                    {t('attendance.stats.review')}
                  </span>
                )}
              </p>
            </div>
          </>
        );
        const shell =
          'bg-white rounded-2xl shadow p-3 sm:p-5 flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 min-w-0';

        return s.action ? (
          <button
            key={s.label}
            type="button"
            onClick={s.action}
            className={`${shell} text-left cursor-pointer hover:shadow-md transition-shadow`}
          >
            {body}
          </button>
        ) : (
          <div key={s.label} className={shell}>
            {body}
          </div>
        );
      })}
    </div>
  );
}

/**
 * One employee's period aggregate as a card, for the same reason as
 * `RecordCard`: nine numeric columns do not survive a phone. The four day
 * counts stay on one row so they remain scannable down the list.
 */
function SummaryCard({ item: i }: { item: AttendanceSummaryItem }) {
  const { t } = useTranslation();
  const counts = [
    { label: t('attendance.stats.present'), value: i.present_days, color: 'text-emerald-700' },
    { label: t('attendance.stats.late'), value: i.late_days, color: 'text-amber-700' },
    { label: attStatusLabel('absent'), value: i.absent_days, color: 'text-red-700' },
    { label: t('attendance.stats.half'), value: i.half_days, color: 'text-blue-700' },
  ];

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-medium text-gray-900 line-clamp-2">{i.full_name}</p>
          <p className="text-xs text-gray-400 truncate">
            {i.employee_number}
            {i.department ? ` · ${i.department}` : ''}
          </p>
        </div>
        {i.unchecked_out_days > 0 && (
          <span
            title={t('attendance.summary.noCheckout')}
            className="shrink-0 inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-0.5 text-xs font-medium text-amber-700"
          >
            <AlertTriangle className="w-3.5 h-3.5" />
            {t('attendance.summary.openDays', { count: i.unchecked_out_days })}
          </span>
        )}
      </div>

      <div className="mt-3 grid grid-cols-4 gap-2">
        {counts.map((c) => (
          <div key={c.label} className="rounded-lg bg-gray-50 px-2 py-1.5 text-center">
            <p className={`text-base font-semibold tabular-nums ${c.color}`}>{c.value}</p>
            <p className="text-[11px] text-gray-500">{c.label}</p>
          </div>
        ))}
      </div>

      <div className="mt-2.5 flex items-center gap-4 text-xs text-gray-600">
        <span className="tabular-nums">
          {t('attendance.summary.hoursWorked', { value: Number(i.total_hours).toFixed(1) })}
        </span>
        <span className="tabular-nums">
          <Timer className="w-3 h-3 inline text-amber-600" />{' '}
          {t('attendance.summary.hoursOt', { value: Number(i.overtime_hours).toFixed(1) })}
        </span>
      </div>
    </div>
  );
}

/**
 * Per-employee aggregate over a date range — the view HR actually needs each
 * payroll cycle. The endpoint always supported the range and filters; nothing
 * consumed them until now (the tiles above only ever asked for "today").
 */
export function SummaryTab({ today, canExport }: { today: string; canExport: boolean }) {
  const { t } = useTranslation();
  const [range, setRange] = useState({ date_from: firstOfMonth(today), date_to: today });
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState('');

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['attendance-summary', range],
    queryFn: () => getAttendanceSummary(range),
    placeholderData: keepPreviousData,
  });
  const items = data ?? [];

  return (
    <div className="bg-white rounded-2xl shadow">
      <div className="flex flex-wrap items-center gap-2.5 sm:gap-3 p-4 sm:p-5 border-b border-gray-100">
        <div className="flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600">
          <Calendar className="w-4 h-4" /> {t('attendance.summary.period')}
        </div>
        <input
          type="date"
          value={range.date_from}
          onChange={(e) => setRange((p) => ({ ...p, date_from: e.target.value }))}
          className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
        <input
          type="date"
          value={range.date_to}
          onChange={(e) => setRange((p) => ({ ...p, date_to: e.target.value }))}
          className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
        {canExport && (
          <button
            onClick={async () => {
              setExporting(true);
              setExportError('');
              try {
                await downloadAttendanceCsv({
                  date_from: range.date_from,
                  date_to: range.date_to,
                });
              } catch (e) {
                setExportError(getErrorMessage(e, t('attendance.page.exportFailed')));
              } finally {
                setExporting(false);
              }
            }}
            disabled={exporting}
            className="w-full sm:w-auto sm:ml-auto flex items-center justify-center gap-2 whitespace-nowrap text-sm font-medium px-4 py-2 sm:py-1.5 rounded-lg border border-gray-300 hover:bg-gray-50 transition-colors disabled:opacity-50"
          >
            <Download className="w-4 h-4 shrink-0" />
            {exporting ? t('common.exporting') : t('attendance.summary.exportPeriod')}
          </button>
        )}
      </div>

      {exportError && (
        <p className="px-5 py-2 text-sm text-red-600 flex items-center gap-1.5">
          <AlertCircle className="w-3.5 h-3.5" /> {exportError}
        </p>
      )}

      {isLoading ? (
        <div className="flex items-center justify-center h-48">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
        </div>
      ) : isError ? (
        <div className="flex flex-col items-center justify-center h-48 gap-2 text-gray-600">
          <AlertCircle className="w-8 h-8 text-red-400" />
          <p className="text-sm">{t('attendance.summary.loadFailed')}</p>
          <button
            onClick={() => refetch()}
            className="text-sm font-medium text-gray-900 hover:underline"
          >
            {t('common.retry')}
          </button>
        </div>
      ) : (
        <>
          {/* Cards below lg, table above — see SummaryCard. */}
          <div className="divide-y divide-gray-100 lg:hidden">
            {items.map((i) => (
              <SummaryCard key={i.employee_id} item={i} />
            ))}
          </div>

          <div className="hidden lg:block overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100">
                  {[
                    t('common.employee'),
                    t('common.department'),
                    t('attendance.stats.present'),
                    t('attendance.stats.late'),
                    attStatusLabel('absent'),
                    t('attendance.stats.half'),
                    t('attendance.summary.hours'),
                    t('attendance.summary.overtime'),
                    t('attendance.summary.open'),
                  ].map((h) => (
                    <th
                      key={h}
                      className="px-5 py-3 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider whitespace-nowrap"
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-50">
                {items.map((i) => (
                  <tr key={i.employee_id} className="hover:bg-gray-50/50 transition-colors">
                    <td className="px-5 py-3">
                      <div className="font-medium text-gray-900">{i.full_name}</div>
                      <div className="text-xs text-gray-400">{i.employee_number}</div>
                    </td>
                    <td className="px-5 py-3 text-gray-600">
                      {i.department || t('common.unassigned')}
                    </td>
                    <td className="px-5 py-3 tabular-nums text-emerald-700">{i.present_days}</td>
                    <td className="px-5 py-3 tabular-nums text-amber-700">{i.late_days}</td>
                    <td className="px-5 py-3 tabular-nums text-red-700">{i.absent_days}</td>
                    <td className="px-5 py-3 tabular-nums text-blue-700">{i.half_days}</td>
                    <td className="px-5 py-3 tabular-nums text-gray-700">
                      {t('attendance.hoursShort', { value: Number(i.total_hours).toFixed(1) })}
                    </td>
                    <td className="px-5 py-3 tabular-nums text-gray-700">
                      {t('attendance.hoursShort', { value: Number(i.overtime_hours).toFixed(1) })}
                    </td>
                    <td className="px-5 py-3 tabular-nums">
                      {i.unchecked_out_days > 0 ? (
                        <span className="inline-flex items-center gap-1 text-amber-600 font-medium">
                          <AlertTriangle className="w-3.5 h-3.5" />
                          {i.unchecked_out_days}
                        </span>
                      ) : (
                        <span className="text-gray-300">—</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Outside the table wrapper — that wrapper is display:none on phones,
            which would have turned "no results" into a blank panel. */}
          {items.length === 0 && (
            <div className="flex flex-col items-center justify-center h-40 text-gray-400">
              <Calendar className="w-10 h-10 mb-2 opacity-40" />
              <p className="text-sm">{t('attendance.summary.empty')}</p>
            </div>
          )}
        </>
      )}
    </div>
  );
}
