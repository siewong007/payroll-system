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
  Loader2,
  LogOut,
  Timer,
} from 'lucide-react';
import {
  downloadAttendanceCsv,
  getAttendanceSummary,
  type AttendanceSummaryItem,
} from '@/api/attendance';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
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
  const { data, isLoading, isError } = useQuery({
    queryKey: ['attendance-records', 'today-stats', today],
    queryFn: () => getAttendanceSummary({ date_from: today, date_to: today }),
  });
  const items = data ?? [];
  const present = items.reduce((sum, i) => sum + i.present_days, 0);
  const late = items.reduce((sum, i) => sum + i.late_days, 0);
  const stillIn = items.reduce((sum, i) => sum + i.unchecked_out_days, 0);

  if (isLoading) {
    return (
      <div
        role="status"
        aria-busy="true"
        aria-label={t('common.loading')}
        className="grid grid-cols-1 sm:grid-cols-3 gap-3 sm:gap-4 mb-6"
      >
        {[0, 1, 2].map((i) => (
          <Skeleton key={i} className="h-20 w-full rounded-2xl" />
        ))}
      </div>
    );
  }

  if (isError) {
    return (
      <div className="card !p-5 mb-6 flex items-center gap-3 text-sm text-gray-600 dark:text-slate-300">
        <AlertCircle className="w-5 h-5 text-red-500 shrink-0" aria-hidden="true" />
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
      color: 'text-emerald-600 dark:text-emerald-400',
      bg: 'bg-emerald-50 dark:bg-emerald-500/15',
      action: undefined,
    },
    {
      icon: Clock,
      label: t('attendance.stats.late'),
      short: t('attendance.stats.late'),
      value: late,
      color: 'text-amber-600 dark:text-amber-400',
      bg: 'bg-amber-50 dark:bg-amber-500/15',
      action: undefined,
    },
    // Actionable: these are the sessions someone has to chase or correct.
    {
      icon: LogOut,
      label: t('attendance.stats.stillIn'),
      short: t('attendance.stats.stillInShort'),
      value: stillIn,
      color: 'text-blue-600 dark:text-blue-400',
      bg: 'bg-blue-50 dark:bg-blue-500/15',
      action: onShowOpen,
    },
  ];

  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 sm:gap-4 mb-6">
      {tiles.map((s) => {
        const body = (
          <>
            <div className={`${s.bg} ${s.color} p-2.5 sm:p-3 rounded-xl shrink-0 w-fit`}>
              <s.icon className="w-4 h-4 sm:w-5 sm:h-5" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <p className="text-xl sm:text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100 leading-tight">{s.value}</p>
              <p className="text-xs text-gray-500 dark:text-slate-400 leading-tight">
                <span className="sm:hidden">{s.short}</span>
                <span className="hidden sm:inline">{s.label}</span>
                {s.action && s.value > 0 && (
                  <span className="ml-1 text-blue-600 dark:text-blue-400 font-medium">
                    {t('attendance.stats.review')}
                  </span>
                )}
              </p>
            </div>
          </>
        );
        const shell =
          'card !p-4 sm:!p-5 flex items-center gap-3 sm:gap-4 min-w-0';

        return s.action ? (
          <button
            key={s.label}
            type="button"
            onClick={s.action}
            className={`${shell} min-h-11 text-left cursor-pointer hover:shadow-md transition-shadow focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]`}
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
 * `RecordCard`: nine numeric columns do not survive a phone.
 */
function SummaryCard({ item: i }: { item: AttendanceSummaryItem }) {
  const { t } = useTranslation();
  const counts = [
    { label: t('attendance.stats.present'), value: i.present_days, color: 'text-emerald-700 dark:text-emerald-300' },
    { label: t('attendance.stats.late'), value: i.late_days, color: 'text-amber-700 dark:text-amber-300' },
    { label: attStatusLabel('absent'), value: i.absent_days, color: 'text-red-700 dark:text-red-300' },
    { label: t('attendance.stats.half'), value: i.half_days, color: 'text-blue-700 dark:text-blue-300' },
  ];

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-medium text-gray-900 dark:text-slate-100 line-clamp-2">{i.full_name}</p>
          <p className="text-xs text-gray-500 dark:text-slate-400 truncate">
            {i.employee_number}
            {i.department ? ` · ${i.department}` : ''}
          </p>
        </div>
        {i.unchecked_out_days > 0 && (
          <span
            title={t('attendance.summary.noCheckout')}
            className="shrink-0 inline-flex items-center gap-1 rounded-full bg-amber-50 dark:bg-amber-500/15 px-2.5 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300 tabular-nums"
          >
            <AlertTriangle className="w-3.5 h-3.5" aria-hidden="true" />
            {t('attendance.summary.openDays', { count: i.unchecked_out_days })}
          </span>
        )}
      </div>

      <div className="mt-3 grid grid-cols-4 gap-2">
        {counts.map((c) => (
          <div key={c.label} className="rounded-lg bg-gray-50 dark:bg-slate-800/70 px-2 py-1.5 text-center">
            <p className={`text-base font-semibold tabular-nums ${c.color}`}>{c.value}</p>
            <p className="text-[11px] text-gray-500 dark:text-slate-400">{c.label}</p>
          </div>
        ))}
      </div>

      <div className="mt-2.5 flex items-center gap-4 text-xs text-gray-600 dark:text-slate-400">
        <span className="tabular-nums">
          {t('attendance.summary.hoursWorked', { value: Number(i.total_hours).toFixed(1) })}
        </span>
        <span className="tabular-nums">
          <Timer className="w-3 h-3 inline text-amber-600 dark:text-amber-400" />{' '}
          {t('attendance.summary.hoursOt', { value: Number(i.overtime_hours).toFixed(1) })}
        </span>
      </div>
    </div>
  );
}

export function SummaryTab({
  today,
  canExport,
  canView = true,
}: {
  today: string;
  canExport: boolean;
  canView?: boolean;
}) {
  const { t } = useTranslation();
  const [range, setRange] = useState({ date_from: firstOfMonth(today), date_to: today });
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState('');

  const { data, isLoading, isFetching, isError, refetch } = useQuery({
    queryKey: ['attendance-summary', range],
    queryFn: () => getAttendanceSummary(range),
    enabled: canView,
    placeholderData: keepPreviousData,
  });
  const items = data ?? [];

  return (
    <div className="card !p-0 overflow-hidden">
      <div className="flex flex-wrap items-end gap-2.5 sm:gap-3 p-4 sm:p-5 border-b border-gray-100 dark:border-slate-800">
        <div className="flex w-full sm:w-auto items-center gap-2 self-center text-sm text-gray-600 dark:text-slate-300">
          <Calendar className="w-4 h-4" aria-hidden="true" /> {t('attendance.summary.period')}
        </div>
        <label className="flex flex-col gap-1 min-w-[8.5rem] flex-1 sm:flex-none">
          <span className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('attendance.page.from')}</span>
          <input
            type="date"
            value={range.date_from}
            max={range.date_to || undefined}
            onChange={(e) => setRange((p) => ({ ...p, date_from: e.target.value }))}
            aria-label={t('attendance.page.from')}
            className="form-input !w-full min-h-11 sm:min-h-9 !py-1.5 sm:text-xs"
          />
        </label>
        <label className="flex flex-col gap-1 min-w-[8.5rem] flex-1 sm:flex-none">
          <span className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('attendance.page.to')}</span>
          <input
            type="date"
            value={range.date_to}
            min={range.date_from || undefined}
            onChange={(e) => setRange((p) => ({ ...p, date_to: e.target.value }))}
            aria-label={t('attendance.page.to')}
            className="form-input !w-full min-h-11 sm:min-h-9 !py-1.5 sm:text-xs"
          />
        </label>
        {canExport && (
          <button
            type="button"
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
            className="btn-secondary w-full sm:w-auto sm:ml-auto !py-2 sm:!py-1.5 min-h-11 sm:!min-h-9"
          >
            {exporting ? (
              <Loader2 className="w-4 h-4 shrink-0 animate-spin" aria-hidden="true" />
            ) : (
              <Download className="w-4 h-4 shrink-0" aria-hidden="true" />
            )}
            {exporting ? t('common.exporting') : t('attendance.summary.exportPeriod')}
          </button>
        )}
      </div>

      {exportError && (
        <p role="alert" className="px-5 py-2 text-sm text-red-600 dark:text-red-400 flex items-center gap-1.5">
          <AlertCircle className="w-3.5 h-3.5 shrink-0" aria-hidden="true" /> {exportError}
        </p>
      )}

      {isLoading ? (
        <div role="status" aria-busy="true" aria-label={t('common.loading')} className="p-4 sm:p-5 space-y-3">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-14 w-full rounded-xl" />
          ))}
        </div>
      ) : isError ? (
        <EmptyState
          icon={AlertCircle}
          title={t('attendance.summary.loadFailed')}
          action={
            <button type="button" onClick={() => refetch()} className="btn-primary">
              {t('common.retry')}
            </button>
          }
        />
      ) : items.length === 0 ? (
        <EmptyState icon={Calendar} title={t('attendance.summary.empty')} />
      ) : (
        <div className={isFetching && !isLoading ? 'opacity-75 transition-opacity' : undefined}>
          {/* Cards below lg, table above — see SummaryCard. */}
          <div className="divide-y divide-gray-100 dark:divide-slate-800 lg:hidden">
            {items.map((i) => (
              <SummaryCard key={i.employee_id} item={i} />
            ))}
          </div>

          <div className="hidden lg:block overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100 dark:border-slate-800">
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
                      className="px-5 py-3 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap"
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-slate-800/80">
                {items.map((i) => (
                  <tr key={i.employee_id} className="hover:bg-gray-50/70 dark:hover:bg-slate-800/40 transition-colors">
                    <td className="px-5 py-3">
                      <div className="font-medium text-gray-900 dark:text-slate-100">{i.full_name}</div>
                      <div className="text-xs text-gray-500 dark:text-slate-400">{i.employee_number}</div>
                    </td>
                    <td className="px-5 py-3 text-gray-600 dark:text-slate-300">
                      {i.department || t('common.unassigned')}
                    </td>
                    <td className="px-5 py-3 tabular-nums text-emerald-700 dark:text-emerald-300">{i.present_days}</td>
                    <td className="px-5 py-3 tabular-nums text-amber-700 dark:text-amber-300">{i.late_days}</td>
                    <td className="px-5 py-3 tabular-nums text-red-700 dark:text-red-300">{i.absent_days}</td>
                    <td className="px-5 py-3 tabular-nums text-blue-700 dark:text-blue-300">{i.half_days}</td>
                    <td className="px-5 py-3 tabular-nums text-gray-700 dark:text-slate-200">
                      {t('attendance.hoursShort', { value: Number(i.total_hours).toFixed(1) })}
                    </td>
                    <td className="px-5 py-3 tabular-nums text-gray-700 dark:text-slate-200">
                      {t('attendance.hoursShort', { value: Number(i.overtime_hours).toFixed(1) })}
                    </td>
                    <td className="px-5 py-3 tabular-nums">
                      {i.unchecked_out_days > 0 ? (
                        <span className="inline-flex items-center gap-1 text-amber-600 dark:text-amber-400 font-medium">
                          <AlertTriangle className="w-3.5 h-3.5" aria-hidden="true" />
                          {i.unchecked_out_days}
                        </span>
                      ) : (
                        <span className="text-gray-400 dark:text-slate-500">—</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
