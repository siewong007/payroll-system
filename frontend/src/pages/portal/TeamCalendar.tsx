import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getTeamCalendar, getPortalHolidays } from '@/api/portal';
import type { Holiday, TeamLeaveEntry } from '@/types';

const DAY_NAMES = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTH_NAMES = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

// Generate a consistent color for an employee name
function nameColor(name: string): string {
  const colors = [
    'bg-blue-100 text-blue-700',
    'bg-purple-100 text-purple-700',
    'bg-pink-100 text-pink-700',
    'bg-indigo-100 text-indigo-700',
    'bg-teal-100 text-teal-700',
    'bg-orange-100 text-orange-700',
    'bg-cyan-100 text-cyan-700',
    'bg-emerald-100 text-emerald-700',
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

export function TeamCalendar() {
  const now = new Date();
  const [selectedYear, setSelectedYear] = useState(now.getFullYear());
  const [selectedMonth, setSelectedMonth] = useState(now.getMonth() + 1);
  const [activeTab, setActiveTab] = useState<'calendar' | 'holidays'>('calendar');

  const { data: teamLeaves = [] } = useQuery({
    queryKey: ['team-calendar', selectedYear, selectedMonth],
    queryFn: () => getTeamCalendar(selectedYear, selectedMonth),
  });

  const { data: holidays = [] } = useQuery({
    queryKey: ['portal-holidays', selectedYear],
    queryFn: () => getPortalHolidays(selectedYear),
  });

  const prevMonth = () => {
    if (selectedMonth === 1) {
      setSelectedMonth(12);
      setSelectedYear(selectedYear - 1);
    } else {
      setSelectedMonth(selectedMonth - 1);
    }
  };

  const nextMonth = () => {
    if (selectedMonth === 12) {
      setSelectedMonth(1);
      setSelectedYear(selectedYear + 1);
    } else {
      setSelectedMonth(selectedMonth + 1);
    }
  };

  const renderCalendarGrid = () => {
    const firstDay = new Date(selectedYear, selectedMonth - 1, 1);
    const lastDay = new Date(selectedYear, selectedMonth, 0);
    const startDow = firstDay.getDay();
    const daysInMonth = lastDay.getDate();

    // Map holidays by day
    const holidayMap = new Map<number, Holiday[]>();
    holidays
      .filter((h) => {
        const d = new Date(h.date);
        return d.getMonth() + 1 === selectedMonth && d.getFullYear() === selectedYear;
      })
      .forEach((h) => {
        const day = new Date(h.date).getDate();
        if (!holidayMap.has(day)) holidayMap.set(day, []);
        holidayMap.get(day)!.push(h);
      });

    // Map team leaves by day (a leave spans start_date to end_date)
    const leaveMap = new Map<number, TeamLeaveEntry[]>();
    teamLeaves.forEach((entry) => {
      const start = new Date(entry.start_date);
      const end = new Date(entry.end_date);
      for (let d = new Date(Math.max(start.getTime(), firstDay.getTime())); d <= end && d <= lastDay; d.setDate(d.getDate() + 1)) {
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

    const cells = [];
    for (let i = 0; i < startDow; i++) {
      cells.push(<div key={`empty-${i}`} className="min-h-[100px] bg-gray-50 border border-gray-100" />);
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
          className={`min-h-[100px] border border-gray-100 p-1.5 ${
            isWeekend ? 'bg-gray-50' : 'bg-white'
          } ${isToday ? 'ring-2 ring-black ring-inset' : ''}`}
        >
          <div className={`text-xs font-medium mb-0.5 ${isWeekend ? 'text-gray-400' : 'text-gray-700'} ${isToday ? 'font-bold text-black' : ''}`}>
            {day}
          </div>
          {dayHolidays.map((h) => (
            <div
              key={h.id}
              className="text-[10px] px-1 py-0.5 rounded truncate bg-red-100 text-red-700 mb-0.5"
              title={h.name}
            >
              {h.name}
            </div>
          ))}
          {dayLeaves.map((entry) => (
            <div
              key={entry.id}
              className={`text-[10px] px-1 py-0.5 rounded truncate mb-0.5 ${nameColor(entry.employee_name)}`}
              title={`${entry.employee_name} - ${entry.leave_type_name}`}
            >
              {entry.employee_name.split(' ')[0]} - {entry.leave_type_name}
            </div>
          ))}
        </div>
      );
    }

    return (
      <div className="grid grid-cols-7 gap-0">
        {DAY_NAMES.map((name) => (
          <div key={name} className="text-center text-xs font-medium text-gray-500 py-2 bg-gray-50 border border-gray-100">
            {name.slice(0, 3)}
          </div>
        ))}
        {cells}
      </div>
    );
  };

  // Unique team members on leave
  const uniqueMembers = Array.from(new Set(teamLeaves.map((e) => e.employee_name)));

  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">Team Calendar</h1>
        <p className="page-subtitle">View holidays and your team's leave schedule</p>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200">
        {[
          { key: 'calendar' as const, label: 'Team Calendar' },
          { key: 'holidays' as const, label: 'Public Holidays' },
        ].map((t) => (
          <button
            key={t.key}
            onClick={() => setActiveTab(t.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-all ${
              activeTab === t.key ? 'border-gray-900 text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {activeTab === 'calendar' && (
        <>
          <div className="bg-white rounded-2xl border border-gray-200 p-6">
            {/* Month Navigation */}
            <div className="flex items-center justify-between mb-4">
              <button onClick={prevMonth} className="p-2 hover:bg-gray-100 rounded-lg">&larr;</button>
              <div className="text-center">
                <h2 className="text-lg font-semibold">
                  {MONTH_NAMES[selectedMonth - 1]} {selectedYear}
                </h2>
                {teamLeaves.length > 0 && (
                  <p className="text-sm text-gray-500">
                    {uniqueMembers.length} team member{uniqueMembers.length !== 1 ? 's' : ''} on leave
                  </p>
                )}
              </div>
              <button onClick={nextMonth} className="p-2 hover:bg-gray-100 rounded-lg">&rarr;</button>
            </div>
            {renderCalendarGrid()}
            {/* Legend */}
            <div className="flex flex-wrap gap-3 mt-4 text-xs text-gray-500">
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-red-100 border border-red-200" /> Holiday
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
            <div className="bg-white rounded-2xl border border-gray-200">
              <div className="px-6 py-4 border-b border-gray-100">
                <h3 className="text-sm font-semibold text-gray-900">Team Leave This Month</h3>
              </div>
              <div className="divide-y divide-gray-100">
                {teamLeaves.map((entry) => (
                  <div key={entry.id} className="flex items-center justify-between px-6 py-3">
                    <div className="flex items-center gap-3">
                      <div className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold ${nameColor(entry.employee_name)}`}>
                        {entry.employee_name[0]}
                      </div>
                      <div>
                        <p className="text-sm font-medium text-gray-900">{entry.employee_name}</p>
                        <p className="text-xs text-gray-500">{entry.leave_type_name}</p>
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-sm text-gray-700">
                        {new Date(entry.start_date).toLocaleDateString('en-MY', { day: 'numeric', month: 'short' })}
                        {entry.start_date !== entry.end_date && (
                          <> &ndash; {new Date(entry.end_date).toLocaleDateString('en-MY', { day: 'numeric', month: 'short' })}</>
                        )}
                      </p>
                      <p className="text-xs text-gray-400">{Number(entry.days)} day{Number(entry.days) !== 1 ? 's' : ''}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}

      {activeTab === 'holidays' && (
        <div className="bg-white rounded-2xl border border-gray-200">
          <div className="p-4 border-b border-gray-100 flex items-center gap-3">
            <label className="text-sm font-medium text-gray-700">Year:</label>
            <select
              value={selectedYear}
              onChange={(e) => setSelectedYear(Number(e.target.value))}
              className="border border-gray-300 rounded-lg px-3 py-1.5 text-sm"
            >
              {[selectedYear - 1, selectedYear, selectedYear + 1].map((y) => (
                <option key={y} value={y}>{y}</option>
              ))}
            </select>
          </div>
          <div className="divide-y divide-gray-100">
            {holidays.length === 0 && (
              <div className="p-8 text-center text-sm text-gray-400">
                No holidays for {selectedYear}
              </div>
            )}
            {holidays.map((h) => {
              const d = new Date(h.date);
              const isPast = d < now;
              return (
                <div key={h.id} className={`flex items-center gap-4 px-6 py-4 ${isPast ? 'opacity-50' : ''}`}>
                  <div className="text-center min-w-[50px]">
                    <div className="text-xs text-gray-400 uppercase">
                      {MONTH_NAMES[d.getMonth()].slice(0, 3)}
                    </div>
                    <div className="text-xl font-bold text-gray-900">{d.getDate()}</div>
                  </div>
                  <div className="flex-1">
                    <p className="text-sm font-medium text-gray-900">{h.name}</p>
                    <p className="text-xs text-gray-400">
                      {DAY_NAMES[d.getDay()]}
                      {h.description ? ` \u2022 ${h.description}` : ''}
                    </p>
                  </div>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                    h.holiday_type === 'public_holiday'
                      ? 'bg-red-100 text-red-700'
                      : h.holiday_type === 'company_holiday'
                      ? 'bg-blue-100 text-blue-700'
                      : h.holiday_type === 'state_holiday'
                      ? 'bg-yellow-100 text-yellow-700'
                      : 'bg-green-100 text-green-700'
                  }`}>
                    {h.holiday_type.replace('_', ' ')}
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
