import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertCircle, ShieldAlert, Smartphone, Users } from 'lucide-react';
import { getAttendanceAnomalies } from '@/api/attendance';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';

interface AttendanceAnomaliesTabProps {
  today: string;
  onViewFlaggedRecords: () => void;
  onReviewDevices: () => void;
}

export function AttendanceAnomaliesTab({
  today,
  onViewFlaggedRecords,
  onReviewDevices,
}: AttendanceAnomaliesTabProps) {
  const { t } = useTranslation();
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['attendance-anomalies', dateFrom, dateTo],
    queryFn: () =>
      getAttendanceAnomalies({
        ...(dateFrom ? { date_from: dateFrom } : {}),
        ...(dateTo ? { date_to: dateTo } : {}),
      }),
  });

  if (isLoading) {
    return (
      <div className="space-y-4">
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-24 rounded-xl" />
          ))}
        </div>
        <Skeleton className="h-56 rounded-xl" />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <EmptyState
        icon={AlertCircle}
        title={t('attendance.anomalies.loadFailed')}
        action={
          <button type="button" onClick={() => void refetch()} className="btn-primary">
            {t('common.retry')}
          </button>
        }
      />
    );
  }

  return (
    <div className="space-y-6">
      {/* Date range filter */}
      <div className="card p-4 flex flex-wrap items-end justify-between gap-3">
        <div className="flex flex-wrap items-end gap-3">
          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-gray-500 dark:text-slate-400">
              {t('attendance.page.from')}
            </span>
            <input
              type="date"
              value={dateFrom || data.date_from}
              max={dateTo || data.date_to || today}
              onChange={(e) => setDateFrom(e.target.value)}
              className="form-input min-h-9 !py-1 text-xs"
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-gray-500 dark:text-slate-400">
              {t('attendance.page.to')}
            </span>
            <input
              type="date"
              value={dateTo || data.date_to}
              min={dateFrom || data.date_from}
              max={today}
              onChange={(e) => setDateTo(e.target.value)}
              className="form-input min-h-9 !py-1 text-xs"
            />
          </label>
        </div>
        {(dateFrom || dateTo) && (
          <button
            type="button"
            onClick={() => {
              setDateFrom('');
              setDateTo('');
            }}
            className="text-xs text-gray-500 dark:text-slate-400 hover:underline"
          >
            {t('common.clear')}
          </button>
        )}
      </div>

      {/* Summary KPIs */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <button
          type="button"
          onClick={onViewFlaggedRecords}
          className="kpi-card text-left hover:border-[var(--accent-1)]/50 transition-colors"
        >
          <p className="text-xs font-medium text-[var(--muted-foreground)]">
            {t('attendance.anomalies.flaggedRecords')}
          </p>
          <p className="mt-1 text-2xl font-bold tabular-nums text-amber-600 dark:text-amber-400">
            {data.summary.flagged_records}
          </p>
        </button>

        <div className="kpi-card">
          <p className="text-xs font-medium text-[var(--muted-foreground)]">
            {t('attendance.anomalies.sharedDeviceIncidents')}
          </p>
          <p className="mt-1 text-2xl font-bold tabular-nums text-red-600 dark:text-red-400">
            {data.summary.shared_device_incidents}
          </p>
        </div>

        <div className="kpi-card">
          <p className="text-xs font-medium text-[var(--muted-foreground)]">
            {t('attendance.anomalies.pairedClusters')}
          </p>
          <p className="mt-1 text-2xl font-bold tabular-nums text-[var(--accent-1)]">
            {data.summary.paired_clusters}
          </p>
        </div>

        <button
          type="button"
          onClick={onReviewDevices}
          className="kpi-card text-left hover:border-[var(--accent-1)]/50 transition-colors"
        >
          <p className="text-xs font-medium text-[var(--muted-foreground)]">
            {t('attendance.anomalies.pendingDevices')}
          </p>
          <p className="mt-1 text-2xl font-bold tabular-nums text-sky-600 dark:text-sky-400">
            {data.summary.pending_devices}
          </p>
        </button>
      </div>

      {/* Shared Devices / Same Browser */}
      <div className="card !p-0 overflow-hidden">
        <div className="px-5 py-3.5 border-b border-[var(--border)] flex items-center gap-2">
          <Smartphone className="w-4 h-4 text-red-500" aria-hidden="true" />
          <h3 className="font-semibold text-[var(--foreground)]">
            {t('attendance.anomalies.sharedDeviceHeading')}
          </h3>
        </div>
        {data.shared_device_events.length === 0 ? (
          <div className="p-6 text-sm text-[var(--muted-foreground)]">
            {t('attendance.anomalies.noSharedDevice')}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--border)] text-left text-xs font-semibold text-[var(--muted-foreground)]">
                  <th className="px-5 py-2.5">{t('common.date')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.deviceTag')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.employeesInvolved')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border)]">
                {data.shared_device_events.map((ev, idx) => (
                  <tr key={`${ev.local_date}-${ev.device_tag}-${idx}`}>
                    <td className="px-5 py-3 tabular-nums whitespace-nowrap">{ev.local_date}</td>
                    <td className="px-5 py-3 font-mono text-xs text-[var(--muted-foreground)]">
                      {ev.device_tag.slice(0, 18)}
                    </td>
                    <td className="px-5 py-3">
                      <div className="flex flex-wrap gap-1.5">
                        {ev.employees.map((emp) => (
                          <span
                            key={emp.employee_id}
                            className="inline-flex items-center gap-1 rounded-full bg-red-50 dark:bg-red-500/15 text-red-700 dark:text-red-300 px-2.5 py-0.5 text-xs font-medium"
                          >
                            {emp.full_name} ({emp.employee_number})
                          </span>
                        ))}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Paired Co-Check-In Clusters */}
      <div className="card !p-0 overflow-hidden">
        <div className="px-5 py-3.5 border-b border-[var(--border)] flex items-center gap-2">
          <Users className="w-4 h-4 text-[var(--accent-1)]" aria-hidden="true" />
          <h3 className="font-semibold text-[var(--foreground)]">
            {t('attendance.anomalies.pairedHeading')}
          </h3>
        </div>
        {data.paired_checkins.length === 0 ? (
          <div className="p-6 text-sm text-[var(--muted-foreground)]">
            {t('attendance.anomalies.noPairedClusters')}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--border)] text-left text-xs font-semibold text-[var(--muted-foreground)]">
                  <th className="px-5 py-2.5">{t('attendance.anomalies.pairCol')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.pairedDaysCol')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.pairedCheckoutsCol')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.avgGapCol')}</th>
                  <th className="px-5 py-2.5">{t('attendance.anomalies.lastDateCol')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border)]">
                {data.paired_checkins.map((pair) => (
                  <tr key={`${pair.employee_a_id}-${pair.employee_b_id}`}>
                    <td className="px-5 py-3">
                      <div className="flex items-center gap-1.5 font-medium text-[var(--foreground)]">
                        <ShieldAlert className="w-3.5 h-3.5 text-amber-500 shrink-0" />
                        <span>
                          {pair.employee_a_name} ({pair.employee_a_number}) ↔ {pair.employee_b_name} ({pair.employee_b_number})
                        </span>
                      </div>
                    </td>
                    <td className="px-5 py-3 tabular-nums font-semibold">{pair.paired_days}</td>
                    <td className="px-5 py-3 tabular-nums">{pair.paired_checkout_days}</td>
                    <td className="px-5 py-3 tabular-nums">{pair.avg_gap_seconds}s</td>
                    <td className="px-5 py-3 tabular-nums">{pair.last_paired_date}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
