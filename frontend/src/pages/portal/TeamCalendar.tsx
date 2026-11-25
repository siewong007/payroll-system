import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, ChevronLeft, ChevronRight } from 'lucide-react';
import { getTeamCalendar, getPortalHolidays } from '@/api/portal';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
import type { Holiday, TeamLeaveEntry } from '@/types';
import { formatDateDayMonth, monthName, weekdayName } from '@/lib/format';
import { getErrorMessage } from '@/lib/utils';

function parseLocalDate(dateStr: string): Date {
  const [y, m, d] = dateStr.slice(0, 10).split('-').map(Number);
  return new Date(y, m - 1, d);
}

// Generate a consistent color for an employee name
function nameColor(name: string): string {
  const colors = [
    'bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300',
    'bg-purple-100 text-purple-700 dark:bg-purple-500/20 dark:text-purple-300',
    'bg-pink-100 text-pink-700 dark:bg-pink-500/20 dark:text-pink-300',
    'bg-indigo-100 text-indigo-700 dark:bg-indigo-500/20 dark:text-indigo-300',
    'bg-teal-100 text-teal-700 dark:bg-teal-500/20 dark:text-teal-300',
    'bg-orange-100 text-orange-700 dark:bg-orange-500/20 dark:text-orange-300',
    'bg-cyan-100 text-cyan-700 dark:bg-cyan-500/20 dark:text-cyan-300',
    'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300',
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

export function TeamCalendar() {
  const { t, i18n } = useTranslation();
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  const dayNames = Array.from({ length: 7 }, (_, i) => weekdayName(i, 'long', i18n.language));
  const dayNamesShort = Array.from({ length: 7 }, (_, i) => weekdayName(i, 'short', i18n.language));
  const monthNames = Array.from({ length: 12 }, (_, i) => monthName(i + 1, 'long', i18n.language));
  const monthNamesShort = Array.from({ length: 12 }, (_, i) => monthName(i + 1, 'short', i18n.language));

  const [selectedYear, setSelectedYear] = useState(now.getFullYear());
  const [selectedMonth, setSelectedMonth] = useState(now.getMonth() + 1);
  const [selectedDay, setSelectedDay] = useState<number>(now.getDate());
  const [activeTab, setActiveTab] = useState<'calendar' | 'holidays'>('calendar');

  const {
    data: teamLeaves = [],
    isLoading: leavesLoading,
    isError: leavesError,
    error: leavesErr,
    refetch: refetchLeaves,
  } = useQuery({
    queryKey: ['team-calendar', selectedYear, selectedMonth],
    queryFn: () => getTeamCalendar(selectedYear, selectedMonth),
  });

  const {
    data: holidays = [],
    isLoading: holidaysLoading,
    isError: holidaysError,
    error: holidaysErr,
    refetch: refetchHolidays,
  } = useQuery({
    queryKey: ['portal-holidays', selectedYear],
    queryFn: () => getPortalHolidays(selectedYear),
  });

  const prevMonth = () => {
    setSelectedDay(1);
    if (selectedMonth === 1) {
      setSelectedMonth(12);
      setSelectedYear(selectedYear - 1);
    } else {
      setSelectedMonth(selectedMonth - 1);
    }
  };

  const nextMonth = () => {
    setSelectedDay(1);
    if (selectedMonth === 12) {
      setSelectedMonth(1);
      setSelectedYear(selectedYear + 1);
    } else {
      setSelectedMonth(selectedMonth + 1);
    }
  };

  const firstDay = new Date(selectedYear, selectedMonth - 1, 1);
  const lastDay = new Date(selectedYear, selectedMonth, 0);
  const startDow = firstDay.getDay();
  const daysInMonth = lastDay.getDate();
  const clampedSelectedDay = Math.min(selectedDay, daysInMonth);

  // Map holidays by local calendar day
  const holidayMap = new Map<number, Holiday[]>();
  holidays.forEach((h) => {
    const d = parseLocalDate(h.date);
    if (d.getMonth() + 1 === selectedMonth && d.getFullYear() === selectedYear) {
      const day = d.getDate();
      if (!holidayMap.has(day)) holidayMap.set(day, []);
      holidayMap.get(day)!.push(h);
    }
  });

  // Map team leaves by local calendar day (preserving month-end days in UTC+8 and negative offsets)
  const leaveMap = new Map<number, TeamLeaveEntry[]>();
  teamLeaves.forEach((entry) => {
    const start = parseLocalDate(entry.start_date);
    const end = parseLocalDate(entry.end_date);
    for (
      let d = new Date(Math.max(start.getTime(), firstDay.getTime()));
      d <= end && d <= lastDay;
      d.setDate(d.getDate() + 1)
    ) {
      if (d.getMonth() + 1 === selectedMonth && d.getFullYear() === selectedYear) {
        const day = d.getDate();
        if (!leaveMap.has(day)) leaveMap.set(day, []);
        const existing = leaveMap.get(day)!;
        if (!existing.some((e) => e.id === entry.id)) {
          existing.push(entry);
        }
      }
    }
  });

  const renderDesktopCalendarGrid = () => {
    const cells = [];
    for (let i = 0; i < startDow; i++) {
      cells.push(<div key={`empty-${i}`} className="min-h-[100px] bg-gray-50 dark:bg-slate-800/40 border border-gray-100 dark:border-slate-800" />);
    }
    for (let day = 1; day <= daysInMonth; day++) {
      const dow = (startDow + day - 1) % 7;
      const isWeekend = dow === 0 || dow === 6;
      const dayHolidays = holidayMap.get(day) || [];
      const dayLeaves = leaveMap.get(day) || [];
      const isToday =
        day === now.getDate() &&
        selectedMonth === now.getMonth() + 1 &&
        selectedYear === now.getFullYear();

      cells.push(
        <div
          key={day}
          className={`min-h-[100px] border border-gray-100 dark:border-slate-800 p-1.5 ${
            isWeekend ? 'bg-gray-50 dark:bg-slate-800/40' : 'bg-white dark:bg-slate-900'
          } ${isToday ? 'ring-2 ring-teal-500 ring-inset' : ''}`}
        >
          <div className={`text-xs font-medium mb-0.5 ${isWeekend ? 'text-gray-400 dark:text-slate-500' : 'text-gray-700 dark:text-slate-200'} ${isToday ? 'font-bold text-teal-600 dark:text-teal-300' : ''}`}>
            {day}
          </div>
          {dayHolidays.map((h) => (
            <div
              key={h.id}
              className="text-[10px] px-1 py-0.5 rounded truncate bg-red-100 dark:bg-red-500/20 text-red-700 dark:text-red-300 mb-0.5"
              title={h.name}
            >
              {h.name}
            </div>
          ))}
          {dayLeaves.map((entry) => (
            <div
              key={entry.id}
              className={`text-[10px] px-1 py-0.5 rounded truncate mb-0.5 ${nameColor(entry.employee_name)}`}
              title={t('portal.teamCalendar.leaveTitle', { name: entry.employee_name, type: entry.leave_type_name })}
            >
              {entry.employee_name.split(' ')[0]} - {entry.leave_type_name}
            </div>
          ))}
        </div>
      );
    }

    return (
      <div className="hidden md:grid grid-cols-7 gap-0 rounded-xl overflow-hidden border border-gray-100 dark:border-slate-800">
        {dayNamesShort.map((name) => (
          <div key={name} className="text-center text-xs font-medium text-gray-500 dark:text-slate-400 py-2 bg-gray-50 dark:bg-slate-800/70 border border-gray-100 dark:border-slate-800">
            {name}
          </div>
        ))}
        {cells}
      </div>
    );
  };

  const renderMobileDotCalendar = () => {
    const mobileCells = [];
    for (let i = 0; i < startDow; i++) {
      mobileCells.push(<div key={`m-empty-${i}`} className="h-11" />);
    }
    for (let day = 1; day <= daysInMonth; day++) {
      const dayHolidays = holidayMap.get(day) || [];
      const dayLeaves = leaveMap.get(day) || [];
      const selected = clampedSelectedDay === day;
      const isToday =
        day === now.getDate() &&
        selectedMonth === now.getMonth() + 1 &&
        selectedYear === now.getFullYear();

      mobileCells.push(
        <button
          key={day}
          type="button"
          onClick={() => setSelectedDay(day)}
          aria-pressed={selected}
          aria-label={t('portal.teamCalendar.selectDay', { day })}
          className={`h-11 flex flex-col items-center justify-center rounded-xl text-xs font-medium transition-colors ${
            selected
              ? 'bg-teal-600 text-white font-semibold'
              : isToday
                ? 'ring-1 ring-teal-500 text-teal-700 dark:text-teal-300'
                : 'text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800'
          }`}
        >
          <span>{day}</span>
          {(dayHolidays.length > 0 || dayLeaves.length > 0) && (
            <span className="flex items-center gap-0.5 mt-0.5">
              {dayHolidays.length > 0 && (
                <span className={`w-1.5 h-1.5 rounded-full ${selected ? 'bg-white' : 'bg-red-500'}`} />
              )}
              {dayLeaves.length > 0 && (
                <span className={`w-1.5 h-1.5 rounded-full ${selected ? 'bg-teal-100' : 'bg-teal-500'}`} />
              )}
            </span>
          )}
        </button>
      );
    }

    const selectedHolidays = holidayMap.get(clampedSelectedDay) || [];
    const selectedLeaves = leaveMap.get(clampedSelectedDay) || [];

    return (
      <div className="md:hidden space-y-4">
        <div className="grid grid-cols-7 gap-1 text-center">
          {dayNamesShort.map((name) => (
            <div key={name} className="text-[11px] font-medium text-gray-500 dark:text-slate-400 py-1">
              {name}
            </div>
          ))}
          {mobileCells}
        </div>
        <div className="rounded-xl border border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-800/50 p-3 space-y-2">
          <p className="text-xs font-semibold text-gray-700 dark:text-slate-200">
            {clampedSelectedDay} {monthNamesShort[selectedMonth - 1]} {selectedYear}
          </p>
          {selectedHolidays.length === 0 && selectedLeaves.length === 0 ? (
            <p className="text-xs text-gray-500 dark:text-slate-400">
              {t('portal.teamCalendar.selectedDayEmpty')}
            </p>
          ) : (
            <div className="space-y-1.5">
              {selectedHolidays.map((h) => (
                <div key={h.id} className="text-xs px-2.5 py-1.5 rounded-lg bg-red-100 dark:bg-red-500/20 text-red-700 dark:text-red-300 font-medium">
                  {h.name}
                </div>
              ))}
              {selectedLeaves.map((entry) => (
                <div key={entry.id} className={`text-xs px-2.5 py-1.5 rounded-lg font-medium ${nameColor(entry.employee_name)}`}>
                  {t('portal.teamCalendar.leaveTitle', { name: entry.employee_name, type: entry.leave_type_name })}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  };

  // Unique team members on leave
  const uniqueMembers = Array.from(new Set(teamLeaves.map((e) => e.employee_name)));
  const isActiveTabLoading = activeTab === 'calendar' ? leavesLoading : holidaysLoading;

  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">{t('portal.teamCalendar.title')}</h1>
        <p className="page-subtitle">{t('portal.teamCalendar.subtitle')}</p>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200 dark:border-slate-800 overflow-x-auto">
        {[
          { key: 'calendar' as const, label: t('portal.teamCalendar.tabs.calendar') },
          { key: 'holidays' as const, label: t('portal.teamCalendar.tabs.holidays') },
        ].map((tab) => (
          <button
            key={tab.key}
            type="button"
            onClick={() => setActiveTab(tab.key)}
            className={`min-h-10 px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap transition-all ${
              activeTab === tab.key
                ? 'border-gray-900 dark:border-teal-400 text-gray-900 dark:text-white'
                : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {isActiveTabLoading ? (
        <div
          className="card p-6 space-y-4"
          aria-busy="true"
          aria-label={t('a11y.loading')}
        >
          <Skeleton className="h-6 w-48" />
          <Skeleton className="h-72 w-full" />
        </div>
      ) : activeTab === 'calendar' ? (
        leavesError ? (
          <div className="card">
            <EmptyState
              icon={AlertTriangle}
              title={t('portal.teamCalendar.loadFailed')}
              description={getErrorMessage(leavesErr, t('portal.teamCalendar.loadFailed'))}
              action={
                <button
                  type="button"
                  onClick={() => {
                    void refetchLeaves();
                    if (holidaysError) void refetchHolidays();
                  }}
                  className="btn-primary"
                >
                  {t('common.retry')}
                </button>
              }
            />
          </div>
        ) : (
        <>
          {holidaysError && (
            <div role="alert" className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-amber-200 dark:border-amber-500/30 bg-amber-50 dark:bg-amber-500/10 px-4 py-2.5 text-xs text-amber-800 dark:text-amber-300">
              <span>{getErrorMessage(holidaysErr, t('portal.teamCalendar.holidaysFailed'))}</span>
              <button type="button" onClick={() => void refetchHolidays()} className="font-semibold underline">
                {t('common.retry')}
              </button>
            </div>
          )}

          <div className="card p-4 sm:p-6">
            {/* Month Navigation */}
            <div className="flex items-center justify-between mb-4">
              <button
                type="button"
                onClick={prevMonth}
                aria-label={t('common.previous')}
                className="min-h-10 min-w-10 inline-flex items-center justify-center hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg text-gray-600 dark:text-slate-300"
              >
                <ChevronLeft className="w-5 h-5" />
              </button>
              <div className="text-center">
                <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100">
                  {monthNames[selectedMonth - 1]} {selectedYear}
                </h2>
                {teamLeaves.length > 0 && (
                  <p className="text-sm text-gray-500 dark:text-slate-400">
                    {t('portal.teamCalendar.membersOnLeave', { count: uniqueMembers.length })}
                  </p>
                )}
              </div>
              <button
                type="button"
                onClick={nextMonth}
                aria-label={t('common.next')}
                className="min-h-10 min-w-10 inline-flex items-center justify-center hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg text-gray-600 dark:text-slate-300"
              >
                <ChevronRight className="w-5 h-5" />
              </button>
            </div>
            {renderDesktopCalendarGrid()}
            {renderMobileDotCalendar()}
            {/* Legend */}
            <div className="flex flex-wrap gap-3 mt-4 text-xs text-gray-500 dark:text-slate-400">
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-red-100 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30" /> {t('portal.teamCalendar.holiday')}
              </span>
              {uniqueMembers.map((name) => (
                <span key={name} className="flex items-center gap-1">
                  <span className={`w-3 h-3 rounded ${nameColor(name).split(' ')[0]}`} /> {name.split(' ')[0]}
                </span>
              ))}
            </div>
          </div>

          {/* Leave list for this month */}
          {teamLeaves.length > 0 && (
            <div className="card !p-0 overflow-hidden">
              <div className="px-4 sm:px-6 py-4 border-b border-gray-100 dark:border-slate-800">
                <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('portal.teamCalendar.leaveThisMonth')}</h3>
              </div>
              <div className="divide-y divide-gray-100 dark:divide-slate-800">
                {teamLeaves.map((entry) => (
                  <div key={entry.id} className="flex items-center justify-between gap-3 px-4 sm:px-6 py-3">
                    <div className="flex items-center gap-3 min-w-0">
                      <div className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold shrink-0 ${nameColor(entry.employee_name)}`}>
                        {entry.employee_name[0]}
                      </div>
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{entry.employee_name}</p>
                        <p className="text-xs text-gray-500 dark:text-slate-400 truncate">{entry.leave_type_name}</p>
                      </div>
                    </div>
                    <div className="text-right shrink-0">
                      <p className="text-sm text-gray-700 dark:text-slate-200">
                        {formatDateDayMonth(parseLocalDate(entry.start_date))}
                        {entry.start_date !== entry.end_date && (
                          <> &ndash; {formatDateDayMonth(parseLocalDate(entry.end_date))}</>
                        )}
                      </p>
                      <p className="text-xs text-gray-400 dark:text-slate-400">{t('portal.teamCalendar.days', { count: Number(entry.days) })}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
        )
      ) : holidaysError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('portal.teamCalendar.holidaysFailed')}
            description={getErrorMessage(holidaysErr, t('portal.teamCalendar.holidaysFailed'))}
            action={
              <button
                type="button"
                onClick={() => void refetchHolidays()}
                className="btn-primary"
              >
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : (
        <div className="card !p-0 overflow-hidden">
          <div className="p-4 border-b border-gray-100 dark:border-slate-800 flex items-center gap-3">
            <label className="text-sm font-medium text-gray-700 dark:text-slate-200" htmlFor="portal-holiday-year">{t('common.year')}:</label>
            <select
              id="portal-holiday-year"
              value={selectedYear}
              onChange={(e) => setSelectedYear(Number(e.target.value))}
              className="form-input !w-auto"
            >
              {[selectedYear - 1, selectedYear, selectedYear + 1].map((y) => (
                <option key={y} value={y}>{y}</option>
              ))}
            </select>
          </div>
          <div className="divide-y divide-gray-100 dark:divide-slate-800">
            {holidays.length === 0 && (
              <div className="p-8 text-center text-sm text-gray-400 dark:text-slate-400">
                {t('portal.teamCalendar.noHolidays', { year: selectedYear })}
              </div>
            )}
            {holidays.map((h) => {
              const d = parseLocalDate(h.date);
              const isPast = d < todayStart;
              return (
                <div key={h.id} className={`flex items-center gap-4 px-4 sm:px-6 py-4 ${isPast ? 'opacity-50' : ''}`}>
                  <div className="text-center min-w-[50px]">
                    <div className="text-xs text-gray-400 dark:text-slate-400 uppercase">
                      {monthNamesShort[d.getMonth()]}
                    </div>
                    <div className="text-xl font-bold text-gray-900 dark:text-slate-100">{d.getDate()}</div>
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-gray-900 dark:text-slate-100">{h.name}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400">
                      {dayNames[d.getDay()]}
                      {h.description ? ` • ${h.description}` : ''}
                    </p>
                  </div>
                  <span className={`text-[10px] px-2 py-0.5 rounded-full font-medium shrink-0 ${
                    h.holiday_type === 'public_holiday'
                      ? 'bg-red-100 dark:bg-red-500/20 text-red-700 dark:text-red-300'
                      : h.holiday_type === 'company_holiday'
                      ? 'bg-blue-100 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300'
                      : h.holiday_type === 'state_holiday'
                      ? 'bg-yellow-100 dark:bg-amber-500/20 text-yellow-700 dark:text-amber-300'
                      : 'bg-green-100 dark:bg-emerald-500/20 text-green-700 dark:text-emerald-300'
                  }`}>
                    {t(`enums.holidayType.${h.holiday_type}`, { defaultValue: h.holiday_type })}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
