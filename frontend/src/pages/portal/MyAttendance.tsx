import { useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  LogIn, LogOut, CheckCircle2, Clock, MapPin, Calendar, AlertTriangle, WifiOff, ChevronLeft, ChevronRight,
} from 'lucide-react';
import { getAttendanceMethod, getMyAttendance, getMyTodayAttendance, type AttendanceRecord } from '@/api/attendance';
import { CheckInCard } from '@/components/attendance/CheckInCard';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useAuth } from '@/context/AuthContext';
import { formatZonedTime, zonedDateKey } from '@/lib/attendance';
import { getErrorMessage } from '@/lib/utils';
import { intlLocale } from '@/i18n';

const FALLBACK_TZ = 'Asia/Kuala_Lumpur';

function formatDate(iso: string, timeZone: string) {
  try {
    return new Date(iso).toLocaleDateString(intlLocale(), { timeZone, day: 'numeric', month: 'short', year: 'numeric' });
  } catch {
    return new Date(iso).toLocaleDateString(intlLocale(), { day: 'numeric', month: 'short', year: 'numeric' });
  }
}

const STATUS_STYLE: Record<string, string> = {
  present:  'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300',
  late:     'bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300',
  absent:   'bg-red-100 text-red-700 dark:bg-red-500/20 dark:text-red-300',
  half_day: 'bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300',
};

function HistoryList({
  records,
  activeOpenId,
  timeZone,
  isLoading,
  isFetching,
  isError,
  error,
  hasFilters,
  onClearFilters,
  onRetry,
}: {
  records: AttendanceRecord[];
  activeOpenId: string | null;
  timeZone: string;
  isLoading: boolean;
  isFetching: boolean;
  isError: boolean;
  error: unknown;
  hasFilters: boolean;
  onClearFilters: () => void;
  onRetry: () => void;
}) {
  const { t } = useTranslation();
  const [renderedAtMs] = useState(() => Date.now());

  if (isLoading) {
    return (
      <div
        role="status"
        aria-busy="true"
        aria-label={t('common.loading')}
        className="divide-y divide-gray-100 dark:divide-slate-800"
      >
        {[0, 1, 2].map((idx) => (
          <div key={idx} className="flex items-center justify-between gap-4 py-3.5 px-5">
            <div className="space-y-2 flex-1">
              <Skeleton className="h-4 w-32" />
              <Skeleton className="h-3 w-48" />
            </div>
            <Skeleton className="h-6 w-20 rounded-full" />
          </div>
        ))}
      </div>
    );
  }

  // A failed fetch must not look like an empty history.
  if (isError) {
    return (
      <div className="p-6">
        <EmptyState
          icon={AlertTriangle}
          title={t('portal.myAttendance.loadFailed')}
          description={getErrorMessage(error, t('portal.myAttendance.loadFailed'))}
          action={
            <button type="button" onClick={onRetry} className="btn-primary">
              {t('common.retry')}
            </button>
          }
        />
      </div>
    );
  }

  if (records.length === 0) {
    return (
      <div className="p-6">
        <EmptyState
          icon={Calendar}
          title={t('portal.myAttendance.empty')}
          action={
            hasFilters ? (
              <button type="button" onClick={onClearFilters} className="btn-secondary">
                {t('ux.table.clearFilters')}
              </button>
            ) : undefined
          }
        />
      </div>
    );
  }

  return (
    <div className={`divide-y divide-gray-100 dark:divide-slate-800 ${isFetching ? 'opacity-75 transition-opacity' : ''}`}>
      {records.map(r => {
        const isActiveSession =
          !r.check_out_at &&
          r.status !== 'absent' &&
          (r.id === activeOpenId || (renderedAtMs - new Date(r.check_in_at).getTime()) < 24 * 3600_000);
        const overnightOut =
          Boolean(r.check_out_at) &&
          zonedDateKey(r.check_out_at as string, timeZone) > zonedDateKey(r.check_in_at, timeZone);

        return (
          <div key={r.id} className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 py-3.5 px-4 sm:px-5">
            <div className="flex-1 min-w-[200px]">
              <p className="text-sm font-medium text-gray-900 dark:text-slate-100 tabular-nums">
                {formatDate(r.check_in_at, timeZone)}
              </p>
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 mt-0.5">
                {/* An auto-absent placeholder carries a midnight timestamp that
                    never happened — showing "12:00 AM" reads as a real check-in. */}
                {r.status === 'absent' ? (
                  <span className="text-xs text-gray-500 dark:text-slate-400">{t('portal.myAttendance.noRecord')}</span>
                ) : (
                  <>
                    <span className="text-xs text-gray-500 dark:text-slate-400 flex items-center gap-1 tabular-nums">
                      <LogIn className="w-3 h-3 text-emerald-500 dark:text-emerald-400 shrink-0" aria-hidden="true" />
                      {formatZonedTime(r.check_in_at, timeZone)}
                    </span>
                    {r.check_out_at ? (
                      <span className="text-xs text-gray-500 dark:text-slate-400 flex items-center gap-1 tabular-nums">
                        <LogOut className="w-3 h-3 text-gray-500 dark:text-slate-400 shrink-0" aria-hidden="true" />
                        {formatZonedTime(r.check_out_at, timeZone)}
                        {overnightOut && (
                          <span className="ml-0.5 rounded bg-gray-100 dark:bg-slate-800 px-1 py-px text-[10px] font-medium text-gray-600 dark:text-slate-300 tabular-nums">
                            {t('portal.myAttendance.nextDay')}
                          </span>
                        )}
                      </span>
                    ) : isActiveSession ? (
                      <span className="text-xs text-[var(--accent-1)] font-medium flex items-center gap-1">
                        <Clock className="w-3 h-3 shrink-0" aria-hidden="true" /> {t('portal.myAttendance.inProgress')}
                      </span>
                    ) : (
                      <span className="text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1">
                        <Clock className="w-3 h-3 shrink-0" aria-hidden="true" /> {t('portal.myAttendance.neverCheckedOut')}
                      </span>
                    )}
                    {r.hours_worked && (
                      <span className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                        {t('attendance.hoursShort', { value: r.hours_worked })}
                      </span>
                    )}
                  </>
                )}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2 shrink-0">
              {r.is_outside_geofence && (
                <span
                  role="img"
                  title={t('portal.myAttendance.outsideGeofence')}
                  aria-label={t('portal.myAttendance.outsideGeofence')}
                  className="text-amber-500 dark:text-amber-400 inline-flex items-center"
                >
                  <AlertTriangle className="w-3.5 h-3.5" aria-hidden="true" />
                </span>
              )}
              {r.is_offsite_network && (
                <span
                  role="img"
                  title={t('portal.myAttendance.offsiteNetwork')}
                  aria-label={t('portal.myAttendance.offsiteNetwork')}
                  className="text-amber-600 dark:text-amber-400 inline-flex items-center"
                >
                  <WifiOff className="w-3.5 h-3.5" aria-hidden="true" />
                </span>
              )}
              {r.latitude && r.longitude && (
                <a
                  href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="min-h-11 sm:min-h-8 inline-flex items-center gap-1 px-2.5 rounded-lg text-xs font-medium text-sky-700 dark:text-sky-300 bg-sky-50 dark:bg-sky-500/15 hover:bg-sky-100 dark:hover:bg-sky-500/25 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                  title={t('portal.myAttendance.viewLocation')}
                  aria-label={t('portal.myAttendance.viewLocation')}
                >
                  <MapPin className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                  <span>{t('attendance.records.checkIn')}</span>
                </a>
              )}
              {r.checkout_latitude && r.checkout_longitude && (
                <a
                  href={`https://maps.google.com/?q=${r.checkout_latitude},${r.checkout_longitude}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="min-h-11 sm:min-h-8 inline-flex items-center gap-1 px-2.5 rounded-lg text-xs font-medium text-[var(--accent-1)] bg-[var(--accent-soft)] hover:opacity-85 transition-opacity focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                  title={t('portal.myAttendance.viewCheckoutLocation')}
                  aria-label={t('portal.myAttendance.viewCheckoutLocation')}
                >
                  <MapPin className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                  <span>{t('attendance.records.checkOut')}</span>
                </a>
              )}
              <span className={`px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap ${STATUS_STYLE[r.status] ?? 'bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300'}`}>
                {t(`enums.attendanceStatus.${r.status}`, { defaultValue: r.status })}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function MyAttendance() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const [page, setPage] = useState(1);
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const perPage = 25;

  const hasEmployee = Boolean(user?.employee_id);
  const hasCompany = Boolean(user?.company_id);

  const { data: method } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
    enabled: hasCompany && hasEmployee,
  });
  const { data: todayData } = useQuery({
    queryKey: ['attendance-today'],
    queryFn: getMyTodayAttendance,
    enabled: hasEmployee,
  });
  const {
    data: historyResult,
    isLoading: historyLoading,
    isFetching: historyFetching,
    isError: historyError,
    error: historyErrorObj,
    refetch: refetchHistory,
  } = useQuery({
    queryKey: ['attendance-my', { page, dateFrom, dateTo }],
    queryFn: () => getMyAttendance({
      page,
      per_page: perPage,
      date_from: dateFrom || undefined,
      date_to: dateTo || undefined,
    }),
    enabled: hasEmployee,
    placeholderData: keepPreviousData,
  });

  const history = historyResult?.data ?? [];
  const totalPages = historyResult?.total_pages ?? 1;
  const total = historyResult?.total ?? 0;
  const timeZone = method?.timezone || FALLBACK_TZ;
  const completedCount = history.filter(r => r.status !== 'absent' && r.check_out_at).length;
  const activeOpenId =
    todayData?.record && !todayData.record.check_out_at && todayData.record.status !== 'absent'
      ? todayData.record.id
      : null;
  const hasFilters = Boolean(dateFrom || dateTo);

  return (
    <div className="space-y-6">
      <PageHeader className="mb-0" title={t('portal.myAttendance.title')} subtitle={t('portal.myAttendance.subtitle')} />

      {/* The same card as the portal home — one implementation of check-in,
          so the two surfaces cannot drift apart. */}
      <CheckInCard />

      {hasEmployee && (
        <div className="card !p-0 overflow-hidden">
          <div className="px-4 sm:px-5 py-4 border-b border-gray-100 dark:border-slate-800 flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <Clock className="w-4 h-4 text-gray-500 dark:text-slate-400" aria-hidden="true" />
              <h2 className="font-semibold text-gray-900 dark:text-slate-100 text-sm">{t('portal.myAttendance.historyTitle')}</h2>
            </div>
            {!historyError && history.length > 0 && (
              <span className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400" aria-hidden="true" />
                {t('portal.myAttendance.completed', { count: completedCount })}
              </span>
            )}
            <div className="w-full sm:w-auto sm:ml-auto flex flex-wrap items-end gap-2">
              <label className="flex flex-col gap-1 flex-1 sm:flex-none min-w-[8.5rem]" htmlFor="my-att-from">
                <span className="text-[11px] font-medium text-gray-500 dark:text-slate-400">{t('portal.myAttendance.filterFrom')}</span>
                <input
                  id="my-att-from"
                  type="date"
                  value={dateFrom}
                  max={dateTo || undefined}
                  onChange={(e) => { setDateFrom(e.target.value); setPage(1); }}
                  className="form-input !w-auto min-h-11 sm:min-h-8 sm:!py-1 sm:text-xs tabular-nums"
                />
              </label>
              <label className="flex flex-col gap-1 flex-1 sm:flex-none min-w-[8.5rem]" htmlFor="my-att-to">
                <span className="text-[11px] font-medium text-gray-500 dark:text-slate-400">{t('portal.myAttendance.filterTo')}</span>
                <input
                  id="my-att-to"
                  type="date"
                  value={dateTo}
                  min={dateFrom || undefined}
                  onChange={(e) => { setDateTo(e.target.value); setPage(1); }}
                  className="form-input !w-auto min-h-11 sm:min-h-8 sm:!py-1 sm:text-xs tabular-nums"
                />
              </label>
              {hasFilters && (
                <button
                  type="button"
                  onClick={() => { setDateFrom(''); setDateTo(''); setPage(1); }}
                  className="min-h-11 sm:min-h-8 px-2.5 text-xs font-medium text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-slate-100 underline rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  {t('common.clear')}
                </button>
              )}
            </div>
          </div>

          <HistoryList
            records={history}
            activeOpenId={activeOpenId}
            timeZone={timeZone}
            isLoading={historyLoading}
            isFetching={historyFetching}
            isError={historyError}
            error={historyErrorObj}
            hasFilters={hasFilters}
            onClearFilters={() => { setDateFrom(''); setDateTo(''); setPage(1); }}
            onRetry={() => void refetchHistory()}
          />

          {totalPages > 1 && (
            <div className="flex items-center justify-between gap-3 px-4 sm:px-5 py-3 border-t border-gray-100 dark:border-slate-800 text-xs text-gray-500 dark:text-slate-400">
              <span className="tabular-nums">
                {t('common.showing', {
                  from: (page - 1) * perPage + 1,
                  to: Math.min(page * perPage, total),
                  total,
                })}
              </span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => setPage(p => Math.max(1, p - 1))}
                  disabled={page <= 1}
                  aria-label={t('common.previousPage')}
                  className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  <ChevronLeft className="w-4 h-4" aria-hidden="true" />
                </button>
                <span className="px-2 font-medium text-gray-700 dark:text-slate-200 tabular-nums">
                  {page} / {totalPages}
                </span>
                <button
                  type="button"
                  onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                  disabled={page >= totalPages}
                  aria-label={t('common.nextPage')}
                  className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  <ChevronRight className="w-4 h-4" aria-hidden="true" />
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
