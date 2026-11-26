import { useEffect, useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Download,
  Link as LinkIcon,
  Loader2,
  Plus,
  Upload,
} from 'lucide-react';
import {
  getHolidays,
  createHoliday,
  updateHoliday,
  deleteHoliday,
  getWorkingDays,
  updateWorkingDays,
  getMonthCalendar,
  importIcs,
  importIcsFile,
} from '@/api/calendar';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import { formatDateLong, monthName, weekdayName } from '@/lib/format';
import type { Holiday, CreateHolidayRequest } from '@/types';

const DAY_INDICES = [0, 1, 2, 3, 4, 5, 6];
const HOLIDAY_TYPE_VALUES = ['public_holiday', 'company_holiday', 'replacement_leave', 'state_holiday'];

function parseYmd(dateStr: string): { year: number; month: number; day: number } | null {
  const parts = dateStr.split('-');
  if (parts.length !== 3) return null;
  const [year, month, day] = parts.map(Number);
  if (!year || !month || !day) return null;
  return { year, month, day };
}

function holidayChipClass(type: string) {
  if (type === 'public_holiday') {
    return 'bg-red-100 text-red-700 dark:bg-red-500/20 dark:text-red-300';
  }
  if (type === 'company_holiday') {
    return 'bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300';
  }
  if (type === 'replacement_leave') {
    return 'bg-green-100 text-green-700 dark:bg-emerald-500/20 dark:text-emerald-300';
  }
  return 'bg-yellow-100 text-yellow-700 dark:bg-amber-500/20 dark:text-amber-300';
}

function holidayDotClass(type: string) {
  if (type === 'public_holiday') return 'bg-red-500 dark:bg-red-400';
  if (type === 'company_holiday') return 'bg-blue-500 dark:bg-blue-400';
  if (type === 'replacement_leave') return 'bg-green-500 dark:bg-emerald-400';
  return 'bg-yellow-500 dark:bg-amber-400';
}

export function CalendarPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const { can } = usePermissions();
  const canManageCalendar = can('manage_calendar');

  const now = new Date();
  const [selectedYear, setSelectedYear] = useState(now.getFullYear());
  const [selectedMonth, setSelectedMonth] = useState(now.getMonth() + 1);
  const [selectedDay, setSelectedDay] = useState<number | null>(now.getDate());
  const [showHolidayModal, setShowHolidayModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [editingHoliday, setEditingHoliday] = useState<Holiday | null>(null);
  const [deletingHolidayId, setDeletingHolidayId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'calendar' | 'holidays' | 'working-days'>('calendar');
  const isCurrentMonth = selectedYear === now.getFullYear() && selectedMonth === now.getMonth() + 1;
  const queryClient = useQueryClient();

  const {
    data: holidays = [],
    isLoading: loadingHolidays,
    isError: holidaysError,
    error: holidaysErrorObj,
    refetch: refetchHolidays,
  } = useQuery({
    queryKey: ['holidays', selectedYear],
    queryFn: () => getHolidays(selectedYear),
  });

  const {
    data: workingDays = [],
    isLoading: loadingWorkingDays,
    isError: workingDaysError,
    error: workingDaysErrorObj,
    refetch: refetchWorkingDays,
  } = useQuery({
    queryKey: ['working-days'],
    queryFn: getWorkingDays,
  });

  const {
    data: monthCal,
    isLoading: loadingMonthCal,
    isFetching: fetchingMonthCal,
    isError: monthCalError,
    error: monthCalErrorObj,
    refetch: refetchMonthCal,
  } = useQuery({
    queryKey: ['month-calendar', selectedYear, selectedMonth],
    queryFn: () => getMonthCalendar(selectedYear, selectedMonth),
    placeholderData: keepPreviousData,
  });

  const createMutation = useMutation({
    mutationFn: createHoliday,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['portal-holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      setShowHolidayModal(false);
      toast.success(t('calendar.holidaySaved'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('calendar.holidaySaveFailed'))),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateHolidayRequest> }) =>
      updateHoliday(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['portal-holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      setShowHolidayModal(false);
      setEditingHoliday(null);
      toast.success(t('calendar.holidaySaved'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('calendar.holidaySaveFailed'))),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteHoliday,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['portal-holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      toast.success(t('calendar.holidayDeleted'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('calendar.holidayDeleteFailed'))),
    onSettled: () => setDeletingHolidayId(null),
  });

  const workingDaysMutation = useMutation({
    mutationFn: updateWorkingDays,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['working-days'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      toast.success(t('calendar.workingDaysSaved'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('calendar.workingDaysSaveFailed'))),
  });

  const toggleWorkingDay = (dayOfWeek: number) => {
    if (!canManageCalendar || workingDaysMutation.isPending) return;
    const current = workingDays.find((d) => d.day_of_week === dayOfWeek);
    const isWorking = current ? !current.is_working_day : !(dayOfWeek >= 1 && dayOfWeek <= 5);
    const days = DAY_INDICES.map((i) => ({
      day_of_week: i,
      is_working_day: i === dayOfWeek ? isWorking : (workingDays.find((d) => d.day_of_week === i)?.is_working_day ?? (i >= 1 && i <= 5)),
    }));
    workingDaysMutation.mutate({ days });
  };

  const holidayMap = useMemo(() => {
    const map = new Map<number, Holiday[]>();
    const source = monthCal?.holidays ?? holidays.filter((h) => {
      const parsed = parseYmd(h.date);
      return parsed?.month === selectedMonth && parsed?.year === selectedYear;
    });
    source.forEach((h) => {
      const parsed = parseYmd(h.date);
      if (!parsed) return;
      const existing = map.get(parsed.day);
      if (existing) existing.push(h);
      else map.set(parsed.day, [h]);
    });
    return map;
  }, [holidays, monthCal?.holidays, selectedMonth, selectedYear]);

  const isWorkingDay = (dow: number) => {
    const config = workingDays.find((d) => d.day_of_week === dow);
    return config ? config.is_working_day : dow >= 1 && dow <= 5;
  };

  const yearOptions = useMemo(
    () => Array.from({ length: 7 }, (_, idx) => selectedYear - 3 + idx),
    [selectedYear],
  );

  const selectedDayHolidays = selectedDay ? (holidayMap.get(selectedDay) ?? []) : [];

  const handlePrevMonth = () => {
    setSelectedDay(null);
    if (selectedMonth === 1) {
      setSelectedMonth(12);
      setSelectedYear((y) => y - 1);
    } else {
      setSelectedMonth((m) => m - 1);
    }
  };

  const handleNextMonth = () => {
    setSelectedDay(null);
    if (selectedMonth === 12) {
      setSelectedMonth(1);
      setSelectedYear((y) => y + 1);
    } else {
      setSelectedMonth((m) => m + 1);
    }
  };

  const handleGoToday = () => {
    const today = new Date();
    setSelectedYear(today.getFullYear());
    setSelectedMonth(today.getMonth() + 1);
    setSelectedDay(today.getDate());
  };

  // Calendar grid
  const renderCalendarGrid = () => {
    const firstDay = new Date(selectedYear, selectedMonth - 1, 1);
    const lastDay = new Date(selectedYear, selectedMonth, 0);
    const startDow = firstDay.getDay();
    const daysInMonth = lastDay.getDate();

    const cells = [];
    for (let i = 0; i < startDow; i++) {
      cells.push(
        <div
          key={`empty-${i}`}
          className="min-h-12 lg:h-24 bg-gray-50 dark:bg-slate-800/40 border border-gray-100 dark:border-slate-800"
        />,
      );
    }
    for (let day = 1; day <= daysInMonth; day++) {
      const dow = (startDow + day - 1) % 7;
      const dayHolidays = holidayMap.get(day) || [];
      const isOff = !isWorkingDay(dow);
      const isToday =
        day === now.getDate() &&
        selectedMonth === now.getMonth() + 1 &&
        selectedYear === now.getFullYear();
      const isSelected = selectedDay === day;

      cells.push(
        <button
          type="button"
          key={day}
          onClick={() => setSelectedDay(day)}
          aria-pressed={isSelected}
          aria-label={formatDateLong(new Date(selectedYear, selectedMonth - 1, day))}
          className={`min-h-12 lg:h-24 text-left align-top border border-gray-100 dark:border-slate-800 p-1 lg:p-1.5 overflow-hidden flex flex-col justify-between lg:justify-start transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:ring-inset ${
            isSelected
              ? 'bg-indigo-50/70 dark:bg-indigo-500/15 ring-2 ring-[var(--accent-1)] ring-inset'
              : isOff
                ? 'bg-gray-50 dark:bg-slate-800/50'
                : 'bg-white dark:bg-slate-900 hover:bg-gray-50/60 dark:hover:bg-slate-800/40'
          }`}
        >
          <div
            className={`text-xs font-medium leading-none tabular-nums ${
              isToday
                ? 'inline-flex h-6 w-6 items-center justify-center rounded-full bg-gray-900 text-white dark:bg-slate-100 dark:text-slate-900 font-bold'
                : isOff
                  ? 'text-gray-500 dark:text-slate-400'
                  : 'text-gray-700 dark:text-slate-200'
            }`}
          >
            {day}
          </div>

          {/* Mobile & portrait-tablet dot indicators */}
          {dayHolidays.length > 0 && (
            <div className="flex lg:hidden flex-wrap items-center gap-0.5 mt-1" aria-hidden="true">
              {dayHolidays.slice(0, 3).map((h) => (
                <span key={h.id} className={`w-1.5 h-1.5 rounded-full ${holidayDotClass(h.holiday_type)}`} />
              ))}
              {dayHolidays.length > 3 && (
                <span className="text-[9px] text-gray-500 dark:text-slate-400 leading-none tabular-nums">+{dayHolidays.length - 3}</span>
              )}
            </div>
          )}

          {/* Desktop holiday chips */}
          <div className="hidden lg:block mt-1 space-y-0.5 w-full overflow-hidden">
            {dayHolidays.slice(0, 2).map((h) => (
              <div
                key={h.id}
                className={`text-[10px] px-1 py-0.5 rounded truncate ${holidayChipClass(h.holiday_type)}`}
                title={h.name}
              >
                {h.name}
              </div>
            ))}
            {dayHolidays.length > 2 && (
              <div className="text-[10px] text-gray-500 dark:text-slate-400 px-1 truncate tabular-nums">
                {t('calendar.moreCount', { count: dayHolidays.length - 2 })}
              </div>
            )}
          </div>
        </button>,
      );
    }

    const totalSlots = Math.ceil((startDow + daysInMonth) / 7) * 7;
    for (let i = startDow + daysInMonth; i < totalSlots; i++) {
      cells.push(
        <div
          key={`trailing-${i}`}
          className="min-h-12 lg:h-24 bg-gray-50 dark:bg-slate-800/40 border border-gray-100 dark:border-slate-800"
        />,
      );
    }

    return (
      <div className="grid grid-cols-7 gap-0 rounded-xl overflow-hidden border border-gray-200 dark:border-slate-800">
        {DAY_INDICES.map((dow) => (
          <div key={dow} className="text-center text-xs font-medium text-gray-500 dark:text-slate-400 py-2 bg-gray-50 dark:bg-slate-800/80 border-b border-gray-200 dark:border-slate-800 truncate px-0.5">
            <span className="lg:hidden">{weekdayName(dow, 'narrow')}</span>
            <span className="hidden lg:inline">{weekdayName(dow)}</span>
          </div>
        ))}
        {cells}
      </div>
    );
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('calendar.title')}
        subtitle={t('calendar.subtitle')}
        actions={
          canManageCalendar ? (
            <>
              <button
                type="button"
                onClick={() => setShowImportModal(true)}
                className="btn-secondary w-full sm:w-auto"
              >
                <Download className="w-4 h-4" />
                {t('calendar.importGoogle')}
              </button>
              <button
                type="button"
                onClick={() => {
                  setEditingHoliday(null);
                  createMutation.reset();
                  updateMutation.reset();
                  setShowHolidayModal(true);
                }}
                className="btn-primary w-full sm:w-auto"
              >
                <Plus className="w-4 h-4" />
                {t('calendar.addHoliday')}
              </button>
            </>
          ) : undefined
        }
      />

      {/* Tabs */}
      <div
        role="tablist"
        aria-label={t('calendar.title')}
        className="flex gap-1 bg-gray-100 dark:bg-slate-800/70 p-1 rounded-xl w-fit max-w-full overflow-x-auto scrollbar-thin"
      >
        {(['calendar', 'holidays', 'working-days'] as const).map((tab) => (
          <button
            type="button"
            role="tab"
            key={tab}
            aria-selected={activeTab === tab}
            onClick={() => setActiveTab(tab)}
            className={`min-h-11 sm:min-h-9 px-4 py-2 rounded-lg text-sm font-medium whitespace-nowrap shrink-0 transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              activeTab === tab
                ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-200'
            }`}
          >
            {t(`calendar.tabs.${tab === 'working-days' ? 'workingDays' : tab}`)}
          </button>
        ))}
      </div>

      {activeTab === 'calendar' && (
        <div className="card p-3 sm:p-6">
          {loadingMonthCal && !monthCal ? (
            <div role="status" aria-label={t('common.loading')} className="space-y-4">
              <div className="flex items-center justify-between gap-2">
                <Skeleton className="h-9 w-9 rounded-lg" />
                <Skeleton className="h-6 w-40" />
                <Skeleton className="h-9 w-9 rounded-lg" />
              </div>
              <div className="grid grid-cols-7 gap-2">
                {Array.from({ length: 35 }).map((_, idx) => (
                  <Skeleton key={idx} className="h-12 lg:h-24 w-full rounded-lg" />
                ))}
              </div>
            </div>
          ) : monthCalError ? (
            <EmptyState
              icon={AlertTriangle}
              title={t('calendar.loadFailed')}
              description={getErrorMessage(monthCalErrorObj, t('calendar.loadFailed'))}
              action={
                <button type="button" onClick={() => refetchMonthCal()} className="btn-primary">
                  {t('common.retry')}
                </button>
              }
            />
          ) : (
            <>
              {/* Month Navigation */}
              <div className="flex items-center justify-between gap-2 mb-4">
                <button
                  type="button"
                  onClick={handlePrevMonth}
                  aria-label={t('calendar.previousMonth')}
                  title={t('calendar.previousMonth')}
                  className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] shrink-0"
                >
                  <ChevronLeft className="w-4 h-4" aria-hidden="true" />
                </button>
                <div className="text-center min-w-0 flex-1">
                  <div className="flex items-center justify-center gap-2 flex-wrap">
                    <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 tabular-nums">
                      {monthName(selectedMonth)} {selectedYear}
                    </h2>
                    {!isCurrentMonth && (
                      <button
                        type="button"
                        onClick={handleGoToday}
                        className="min-h-11 sm:min-h-7 px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                      >
                        {t('common.today')}
                      </button>
                    )}
                  </div>
                  {monthCal && (
                    <p className="text-xs sm:text-sm text-gray-500 dark:text-slate-400 tabular-nums">
                      {t('calendar.workingDaysCount', { count: monthCal.working_days })} &middot; {t('calendar.holidaysCount', { count: monthCal.holidays.length })}
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  onClick={handleNextMonth}
                  aria-label={t('calendar.nextMonth')}
                  title={t('calendar.nextMonth')}
                  className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] shrink-0"
                >
                  <ChevronRight className="w-4 h-4" aria-hidden="true" />
                </button>
              </div>

              <div className={fetchingMonthCal ? 'opacity-70 transition-opacity' : undefined}>
                {renderCalendarGrid()}
              </div>

              {/* Selected day agenda */}
              {selectedDay !== null && (
                <div className="mt-4 rounded-xl border border-gray-200 dark:border-slate-800 bg-gray-50 dark:bg-slate-800/50 p-4">
                  <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
                    <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100 tabular-nums">
                      {formatDateLong(new Date(selectedYear, selectedMonth - 1, selectedDay))}
                    </h3>
                    {(() => {
                      const dow = new Date(selectedYear, selectedMonth - 1, selectedDay).getDay();
                      const working = isWorkingDay(dow);
                      return (
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                          working
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                            : 'bg-gray-200 text-gray-700 dark:bg-slate-700 dark:text-slate-300'
                        }`}>
                          {working ? t('calendar.working') : t('calendar.nonWorkingDay')}
                        </span>
                      );
                    })()}
                  </div>
                  {selectedDayHolidays.length === 0 ? (
                    <p className="text-xs text-gray-500 dark:text-slate-400">
                      {t('calendar.noDayHolidays')}
                    </p>
                  ) : (
                    <div className="space-y-2">
                      {selectedDayHolidays.map((h) => (
                        <div key={h.id} className="flex items-start justify-between gap-3 rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 px-3 py-2">
                          <div className="min-w-0">
                            <p className="text-sm font-medium text-gray-900 dark:text-slate-100 break-words">{h.name}</p>
                            <div className="flex flex-wrap items-center gap-2 mt-0.5">
                              <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${holidayChipClass(h.holiday_type)}`}>
                                {t(`enums.holidayType.${h.holiday_type}`, { defaultValue: h.holiday_type })}
                              </span>
                              {h.state && <span className="text-xs text-gray-500 dark:text-slate-400">{h.state}</span>}
                            </div>
                          </div>
                          {canManageCalendar && (
                            <button
                              type="button"
                              onClick={() => {
                                setEditingHoliday(h);
                                createMutation.reset();
                                updateMutation.reset();
                                setShowHolidayModal(true);
                              }}
                              className="min-h-11 sm:min-h-8 text-xs font-medium text-gray-600 dark:text-slate-300 hover:text-black dark:hover:text-white px-3 py-1 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                            >
                              {t('common.edit')}
                            </button>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* Legend */}
              <div className="flex flex-wrap gap-3 sm:gap-4 mt-4 text-xs text-gray-500 dark:text-slate-400">
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 rounded bg-red-100 dark:bg-red-500/30 border border-red-200 dark:border-red-500/40" aria-hidden="true" /> {t('enums.holidayType.public_holiday')}
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 rounded bg-blue-100 dark:bg-blue-500/30 border border-blue-200 dark:border-blue-500/40" aria-hidden="true" /> {t('enums.holidayType.company_holiday')}
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 rounded bg-green-100 dark:bg-emerald-500/30 border border-green-200 dark:border-emerald-500/40" aria-hidden="true" /> {t('enums.holidayType.replacement_leave')}
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 rounded bg-yellow-100 dark:bg-amber-500/30 border border-amber-300 dark:border-amber-500/40" aria-hidden="true" /> {t('enums.holidayType.state_holiday')}
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 rounded bg-gray-100 dark:bg-slate-800 border border-gray-200 dark:border-slate-700" aria-hidden="true" /> {t('calendar.nonWorkingDay')}
                </span>
              </div>
            </>
          )}
        </div>
      )}

      {activeTab === 'holidays' && (
        <div className="card !p-0 overflow-hidden">
          <div className="p-4 border-b border-gray-100 dark:border-slate-800 flex items-center gap-3">
            <label htmlFor="calendar-holiday-year" className="text-sm font-medium text-gray-700 dark:text-slate-300">
              {t('common.year')}:
            </label>
            <select
              id="calendar-holiday-year"
              aria-label={t('common.year')}
              value={selectedYear}
              onChange={(e) => {
                const nextYear = Number(e.target.value);
                setSelectedYear(nextYear);
                setSelectedDay((prev) =>
                  prev !== null ? Math.min(prev, new Date(nextYear, selectedMonth, 0).getDate()) : null,
                );
              }}
              className="form-input !w-auto min-h-11 sm:min-h-9 py-1.5 tabular-nums"
            >
              {yearOptions.map((y) => (
                <option key={y} value={y}>{y}</option>
              ))}
            </select>
          </div>
          {loadingHolidays ? (
            <div role="status" aria-label={t('common.loading')} className="p-4 sm:p-6 space-y-3">
              {[0, 1, 2, 3].map((idx) => (
                <Skeleton key={idx} className="h-14 w-full rounded-xl" />
              ))}
            </div>
          ) : holidaysError ? (
            <div className="p-6">
              <EmptyState
                icon={AlertTriangle}
                title={t('calendar.loadFailed')}
                description={getErrorMessage(holidaysErrorObj, t('calendar.loadFailed'))}
                action={
                  <button type="button" onClick={() => refetchHolidays()} className="btn-primary">
                    {t('common.retry')}
                  </button>
                }
              />
            </div>
          ) : holidays.length === 0 ? (
            <div className="p-6">
              <EmptyState
                icon={CalendarDays}
                title={t('calendar.noHolidays', { year: selectedYear })}
                action={
                  canManageCalendar ? (
                    <button
                      type="button"
                      onClick={() => {
                        setEditingHoliday(null);
                        createMutation.reset();
                        updateMutation.reset();
                        setShowHolidayModal(true);
                      }}
                      className="btn-primary"
                    >
                      <Plus className="w-4 h-4" aria-hidden="true" />
                      {t('calendar.addHoliday')}
                    </button>
                  ) : undefined
                }
              />
            </div>
          ) : (
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              {holidays.map((h) => {
                const parsed = parseYmd(h.date);
                const isDeletingRow = deletingHolidayId === h.id && deleteMutation.isPending;
                return (
                  <div key={h.id} className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 px-4 sm:px-6 py-4">
                    <div className="flex items-center gap-4 min-w-0">
                      <div className="text-center min-w-[50px] shrink-0">
                        <div className="text-xs text-gray-500 dark:text-slate-400 uppercase">
                          {parsed ? monthName(parsed.month, 'short') : '\u2014'}
                        </div>
                        <div className="text-xl font-bold text-gray-900 dark:text-slate-100 tabular-nums">
                          {parsed ? parsed.day : '\u2014'}
                        </div>
                      </div>
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-gray-900 dark:text-slate-100 break-words">{h.name}</p>
                        <div className="flex flex-wrap items-center gap-2 mt-0.5">
                          <span className={`text-[10px] px-2 py-0.5 rounded-full font-medium ${holidayChipClass(h.holiday_type)}`}>
                            {t(`enums.holidayType.${h.holiday_type}`, { defaultValue: h.holiday_type })}
                          </span>
                          {h.is_recurring && (
                            <span className="text-[11px] text-gray-500 dark:text-slate-400">{t('calendar.recurring')}</span>
                          )}
                          {h.state && (
                            <span className="text-[11px] text-gray-500 dark:text-slate-400">{h.state}</span>
                          )}
                        </div>
                      </div>
                    </div>
                    {canManageCalendar && (
                      <div className="flex items-center gap-2 self-end sm:self-center shrink-0">
                        <button
                          type="button"
                          onClick={() => {
                            setEditingHoliday(h);
                            createMutation.reset();
                            updateMutation.reset();
                            setShowHolidayModal(true);
                          }}
                          className="min-h-11 sm:min-h-8 px-3 py-1.5 text-xs font-medium text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                        >
                          {t('common.edit')}
                        </button>
                        <button
                          type="button"
                          disabled={isDeletingRow}
                          onClick={async () => {
                            const ok = await confirmDialog({
                              description: t('calendar.deleteConfirm', { name: h.name }),
                              variant: 'danger',
                            });
                            if (ok) {
                              setDeletingHolidayId(h.id);
                              deleteMutation.mutate(h.id);
                            }
                          }}
                          className="min-h-11 sm:min-h-8 px-3 py-1.5 text-xs font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 rounded-lg disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] inline-flex items-center gap-1.5"
                        >
                          {isDeletingRow && <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />}
                          {isDeletingRow ? t('common.deleting') : t('common.delete')}
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {activeTab === 'working-days' && (
        <div className="card p-5 sm:p-6">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-2">
            {t('calendar.workingDaysTitle')}
          </h2>
          <p className="text-xs text-gray-500 dark:text-slate-400 mb-6">
            {t('calendar.workingDaysDescription')}
          </p>
          {loadingWorkingDays ? (
            <div role="status" aria-label={t('common.loading')} className="grid grid-cols-2 sm:grid-cols-4 xl:grid-cols-7 gap-3">
              {DAY_INDICES.map((i) => (
                <Skeleton key={i} className="h-20 w-full rounded-xl" />
              ))}
            </div>
          ) : workingDaysError ? (
            <EmptyState
              icon={AlertTriangle}
              title={t('calendar.loadFailed')}
              description={getErrorMessage(workingDaysErrorObj, t('calendar.loadFailed'))}
              action={
                <button type="button" onClick={() => refetchWorkingDays()} className="btn-primary">
                  {t('common.retry')}
                </button>
              }
            />
          ) : (
            <div className="grid grid-cols-2 sm:grid-cols-4 xl:grid-cols-7 gap-3">
              {DAY_INDICES.map((i) => {
                const config = workingDays.find((d) => d.day_of_week === i);
                const isWorking = config ? config.is_working_day : i >= 1 && i <= 5;
                const cardStyle = isWorking
                  ? 'border-black dark:border-slate-200 bg-gray-900 dark:bg-slate-100 text-white dark:text-slate-900'
                  : 'border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-gray-500 dark:text-slate-400';

                if (!canManageCalendar) {
                  return (
                    <div key={i} className={`p-4 rounded-xl border-2 text-center ${cardStyle}`}>
                      <div className="text-sm font-semibold">{weekdayName(i)}</div>
                      <div className="text-xs mt-1">{isWorking ? t('calendar.working') : t('calendar.off')}</div>
                    </div>
                  );
                }

                return (
                  <button
                    type="button"
                    key={i}
                    aria-pressed={isWorking}
                    disabled={workingDaysMutation.isPending}
                    onClick={() => toggleWorkingDay(i)}
                    className={`min-h-11 p-4 rounded-xl border-2 text-center transition-all disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:ring-offset-1 ${cardStyle} ${
                      !isWorking ? 'hover:border-gray-300 dark:hover:border-slate-600' : ''
                    }`}
                  >
                    <div className="text-sm font-semibold">{weekdayName(i)}</div>
                    <div className="text-xs mt-1">{isWorking ? t('calendar.working') : t('calendar.off')}</div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* Holiday Create/Edit Modal */}
      <HolidayModal
        open={showHolidayModal}
        holiday={editingHoliday}
        onClose={() => {
          setShowHolidayModal(false);
          setEditingHoliday(null);
        }}
        onSave={(data) => {
          if (editingHoliday) {
            updateMutation.mutate({ id: editingHoliday.id, data });
          } else {
            createMutation.mutate(data as CreateHolidayRequest);
          }
        }}
        isLoading={createMutation.isPending || updateMutation.isPending}
        serverError={
          editingHoliday
            ? updateMutation.isError
              ? getErrorMessage(updateMutation.error, t('calendar.holidaySaveFailed'))
              : ''
            : createMutation.isError
              ? getErrorMessage(createMutation.error, t('calendar.holidaySaveFailed'))
              : ''
        }
      />

      {/* Google Calendar Import Modal */}
      <ImportIcsModal
        open={showImportModal}
        onClose={() => setShowImportModal(false)}
        onSuccess={() => {
          queryClient.invalidateQueries({ queryKey: ['holidays'] });
          queryClient.invalidateQueries({ queryKey: ['portal-holidays'] });
          queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
        }}
      />
    </div>
  );
}

function emptyHolidayForm(holiday: Holiday | null): CreateHolidayRequest {
  return holiday
    ? {
        name: holiday.name,
        date: holiday.date,
        holiday_type: holiday.holiday_type,
        description: holiday.description || '',
        is_recurring: holiday.is_recurring,
        state: holiday.state || '',
      }
    : {
        name: '',
        date: '',
        holiday_type: 'public_holiday',
        description: '',
        is_recurring: false,
        state: '',
      };
}

function HolidayModal({
  open,
  holiday,
  onClose,
  onSave,
  isLoading,
  serverError,
}: {
  open: boolean;
  holiday: Holiday | null;
  onClose: () => void;
  onSave: (data: CreateHolidayRequest | Partial<CreateHolidayRequest>) => void;
  isLoading: boolean;
  serverError: string;
}) {
  const { t } = useTranslation();
  const formId = useId();
  const [form, setForm] = useState<CreateHolidayRequest>(() => emptyHolidayForm(holiday));
  const [validationError, setValidationError] = useState('');

  useEffect(() => {
    if (!open) return;
    setForm(emptyHolidayForm(holiday));
    setValidationError('');
  }, [holiday, open]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = form.name.trim();
    if (!trimmedName || !form.date) {
      setValidationError(t('calendar.holidayRequired'));
      return;
    }
    setValidationError('');
    if (holiday) {
      const updatePayload: Partial<CreateHolidayRequest> = {
        name: trimmedName,
        holiday_type: form.holiday_type,
        description: form.description?.trim() || '',
        is_recurring: form.is_recurring,
        state: form.state?.trim() || '',
      };
      // Virtual occurrences of a recurring holiday carry the queried year's date;
      // omitting `date` when unchanged preserves the original DB anchor date.
      if (form.date !== holiday.date) {
        updatePayload.date = form.date;
      }
      onSave(updatePayload);
      return;
    }
    onSave({
      ...form,
      name: trimmedName,
      description: form.description?.trim() || '',
      state: form.state?.trim() || '',
    });
  };

  const displayError = validationError || serverError;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={holiday ? t('calendar.editHoliday') : t('calendar.addHoliday')}
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="btn-secondary"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={isLoading}
            className="btn-primary"
          >
            {isLoading ? t('common.saving') : holiday ? t('common.update') : t('common.create')}
          </button>
        </div>
      }
    >
      <form id={formId} onSubmit={handleSubmit} className="space-y-4">
        {displayError && (
          <div role="alert" className="rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300">
            {displayError}
          </div>
        )}
        <FormField label={t('common.name')} required>
          {(aria) => (
            <input
              {...aria}
              value={form.name}
              onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
              required
              className="form-input"
              placeholder={t('calendar.namePlaceholder')}
            />
          )}
        </FormField>
        <FormField label={t('common.date')} required>
          {(aria) => (
            <input
              {...aria}
              type="date"
              value={form.date}
              onChange={(e) => setForm((prev) => ({ ...prev, date: e.target.value }))}
              required
              className="form-input"
            />
          )}
        </FormField>
        <FormField label={t('calendar.type')}>
          {(aria) => (
            <select
              {...aria}
              value={form.holiday_type ?? 'public_holiday'}
              onChange={(e) => setForm((prev) => ({ ...prev, holiday_type: e.target.value }))}
              className="form-input"
            >
              {HOLIDAY_TYPE_VALUES.map((v) => (
                <option key={v} value={v}>{t(`enums.holidayType.${v}`)}</option>
              ))}
            </select>
          )}
        </FormField>
        <FormField label={t('common.description')}>
          {(aria) => (
            <input
              {...aria}
              value={form.description ?? ''}
              onChange={(e) => setForm((prev) => ({ ...prev, description: e.target.value }))}
              className="form-input"
              placeholder={t('teams.descriptionPlaceholder')}
            />
          )}
        </FormField>
        <FormField label={t('calendar.stateOptional')}>
          {(aria) => (
            <input
              {...aria}
              value={form.state ?? ''}
              onChange={(e) => setForm((prev) => ({ ...prev, state: e.target.value }))}
              className="form-input"
              placeholder={t('calendar.statePlaceholder')}
            />
          )}
        </FormField>
        <label className="flex items-center gap-2 min-h-11 sm:min-h-0 cursor-pointer">
          <input
            type="checkbox"
            checked={Boolean(form.is_recurring)}
            onChange={(e) => setForm((prev) => ({ ...prev, is_recurring: e.target.checked }))}
            className="h-4 w-4 rounded border-gray-300 dark:border-slate-700 accent-[var(--accent-1)]"
          />
          <span className="text-sm text-gray-700 dark:text-slate-200">
            {t('calendar.recurringLabel')}
          </span>
        </label>
      </form>
    </Modal>
  );
}

function ImportIcsModal({
  open,
  onClose,
  onSuccess,
}: {
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const { t } = useTranslation();
  const formId = useId();
  const [importMode, setImportMode] = useState<'url' | 'file'>('file');
  const [url, setUrl] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [result, setResult] = useState<{ count: number } | null>(null);
  const [error, setError] = useState('');

  const urlMutation = useMutation({
    mutationFn: importIcs,
    onSuccess: (data) => {
      setResult({ count: data.length });
      onSuccess();
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('calendar.importUrlFailed')));
    },
  });

  const fileMutation = useMutation({
    mutationFn: importIcsFile,
    onSuccess: (data) => {
      setResult({ count: data.length });
      onSuccess();
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('calendar.importFileFailed')));
    },
  });

  const isPending = urlMutation.isPending || fileMutation.isPending;

  const acceptIcsFile = (picked: File) => {
    const lower = picked.name.toLowerCase();
    if (!lower.endsWith('.ics') && !lower.endsWith('.ical')) {
      setError(t('calendar.invalidIcsFile'));
      return;
    }
    setError('');
    setResult(null);
    setFile(picked);
  };

  const handleImport = () => {
    setError('');
    setResult(null);
    if (importMode === 'url') {
      if (!url.trim()) {
        setError(t('calendar.urlRequired'));
        return;
      }
      urlMutation.mutate(url.trim());
    } else {
      if (!file) {
        setError(t('calendar.fileRequired'));
        return;
      }
      fileMutation.mutate(file);
    }
  };

  const handleClose = () => {
    setUrl('');
    setFile(null);
    setDragOver(false);
    setResult(null);
    setError('');
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title={t('calendar.importTitle')}
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button
            type="button"
            onClick={handleClose}
            className="btn-secondary"
          >
            {result ? t('common.done') : t('common.cancel')}
          </button>
          {!result && (
            <button
              type="submit"
              form={formId}
              disabled={isPending}
              className="btn-primary"
            >
              <Download className="w-4 h-4" />
              {isPending ? t('common.importing') : t('calendar.importHolidays')}
            </button>
          )}
        </div>
      }
    >
      <form
        id={formId}
        onSubmit={(e) => {
          e.preventDefault();
          handleImport();
        }}
        className="space-y-4"
      >
        <p className="text-sm text-gray-600 dark:text-slate-300">
          {t('calendar.importDescription')}
        </p>

        {/* Mode Toggle */}
        <div className="flex gap-1 bg-gray-100 dark:bg-slate-800/70 p-1 rounded-xl">
          <button
            type="button"
            aria-pressed={importMode === 'file'}
            onClick={() => { setImportMode('file'); setError(''); }}
            className={`min-h-11 sm:min-h-9 flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              importMode === 'file'
                ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                : 'text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
            }`}
          >
            <Upload className="w-4 h-4" aria-hidden="true" />
            {t('calendar.uploadFile')}
          </button>
          <button
            type="button"
            aria-pressed={importMode === 'url'}
            onClick={() => { setImportMode('url'); setError(''); }}
            className={`min-h-11 sm:min-h-9 flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              importMode === 'url'
                ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                : 'text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
            }`}
          >
            <LinkIcon className="w-4 h-4" aria-hidden="true" />
            {t('calendar.pasteUrl')}
          </button>
        </div>

        {importMode === 'file' ? (
          <div>
            <span className="form-label">{t('calendar.icsFile')}</span>
            <label
              onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={(e) => {
                e.preventDefault();
                setDragOver(false);
                const dropped = e.dataTransfer.files?.[0];
                if (dropped) acceptIcsFile(dropped);
              }}
              className={`flex flex-col items-center justify-center w-full h-32 border-2 border-dashed rounded-xl cursor-pointer transition-all focus-within:ring-2 focus-within:ring-[var(--ring)] ${
                dragOver || file
                  ? 'border-[var(--accent-1)] bg-gray-50 dark:bg-slate-800/70'
                  : 'border-gray-300 dark:border-slate-700 hover:border-gray-400 dark:hover:border-slate-600 hover:bg-gray-50 dark:hover:bg-slate-800/40'
              }`}
            >
              <div className="flex flex-col items-center justify-center pt-5 pb-6 px-4 text-center">
                <Upload className={`w-8 h-8 mb-2 ${file ? 'text-black dark:text-slate-100' : 'text-gray-400 dark:text-slate-500'}`} aria-hidden="true" />
                {file ? (
                  <>
                    <p className="text-sm font-medium text-gray-900 dark:text-slate-100 break-all">{file.name}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5 tabular-nums">{(file.size / 1024).toFixed(1)} KB</p>
                  </>
                ) : (
                  <>
                    {/* i18n-ok: `.ics` is a file extension, not copy */}
                    <p className="text-sm text-gray-600 dark:text-slate-300">{t('calendar.clickToSelect')} <span className="font-medium">.ics</span></p>
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{t('calendar.orDragDrop')}</p>
                  </>
                )}
              </div>
              <input
                type="file"
                accept=".ics,.ical"
                className="sr-only"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  e.currentTarget.value = '';
                  if (f) acceptIcsFile(f);
                }}
              />
            </label>
            <p className="text-xs text-gray-500 dark:text-slate-400 mt-2">
              {t('calendar.icsExportHint')}
            </p>
          </div>
        ) : (
          <div>
            <FormField label={t('calendar.icsUrl')}>
              {(aria) => (
                <input
                  {...aria}
                  type="url"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  className="form-input"
                  placeholder="https://calendar.google.com/calendar/ical/..."
                />
              )}
            </FormField>
            <div className="bg-gray-50 dark:bg-slate-800/60 border border-gray-200 dark:border-slate-700 rounded-lg p-3 mt-2 text-xs text-gray-600 dark:text-slate-300 space-y-1">
              <p className="font-medium text-gray-700 dark:text-slate-200">{t('calendar.icsHowTo')}</p>
              <ol className="list-decimal list-inside space-y-0.5">
                <li>{t('calendar.icsStep1')}</li>
                <li>{t('calendar.icsStep2')}</li>
                <li>{t('calendar.icsStep3')}</li>
              </ol>
            </div>
          </div>
        )}

        {error && (
          <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/30">
            {error}
          </div>
        )}

        {result && (
          <div role="status" className="p-3 bg-emerald-50 dark:bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 text-sm rounded-lg border border-emerald-100 dark:border-emerald-500/30">
            {t('calendar.importSuccess', { count: result.count })}
          </div>
        )}
      </form>
    </Modal>
  );
}
