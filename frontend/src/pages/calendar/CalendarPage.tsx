import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { Download, Upload, Link as LinkIcon } from 'lucide-react';
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
import { Modal } from '@/components/ui/Modal';
import { getErrorMessage } from '@/lib/utils';
import { monthName, weekdayName } from '@/lib/format';
import type { Holiday, CreateHolidayRequest } from '@/types';

const DAY_INDICES = [0, 1, 2, 3, 4, 5, 6];
const HOLIDAY_TYPE_VALUES = ['public_holiday', 'company_holiday', 'replacement_leave', 'state_holiday'];

export function CalendarPage() {
  const { t } = useTranslation();
  const now = new Date();
  const [selectedYear, setSelectedYear] = useState(now.getFullYear());
  const [selectedMonth, setSelectedMonth] = useState(now.getMonth() + 1);
  const [showHolidayModal, setShowHolidayModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [editingHoliday, setEditingHoliday] = useState<Holiday | null>(null);
  const [activeTab, setActiveTab] = useState<'calendar' | 'holidays' | 'working-days'>('calendar');
  const queryClient = useQueryClient();

  const { data: holidays = [] } = useQuery({
    queryKey: ['holidays', selectedYear],
    queryFn: () => getHolidays(selectedYear),
  });

  const { data: workingDays = [] } = useQuery({
    queryKey: ['working-days'],
    queryFn: getWorkingDays,
  });

  const { data: monthCal } = useQuery({
    queryKey: ['month-calendar', selectedYear, selectedMonth],
    queryFn: () => getMonthCalendar(selectedYear, selectedMonth),
  });

  const createMutation = useMutation({
    mutationFn: createHoliday,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      setShowHolidayModal(false);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateHolidayRequest> }) =>
      updateHoliday(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
      setShowHolidayModal(false);
      setEditingHoliday(null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteHoliday,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['holidays'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
    },
  });

  const workingDaysMutation = useMutation({
    mutationFn: updateWorkingDays,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['working-days'] });
      queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
    },
  });

  const toggleWorkingDay = (dayOfWeek: number) => {
    const current = workingDays.find((d) => d.day_of_week === dayOfWeek);
    const isWorking = current ? !current.is_working_day : false;
    const days = DAY_INDICES.map((i) => ({
      day_of_week: i,
      is_working_day: i === dayOfWeek ? isWorking : (workingDays.find((d) => d.day_of_week === i)?.is_working_day ?? (i >= 1 && i <= 5)),
    }));
    workingDaysMutation.mutate({ days });
  };

  // Calendar grid
  const renderCalendarGrid = () => {
    const firstDay = new Date(selectedYear, selectedMonth - 1, 1);
    const lastDay = new Date(selectedYear, selectedMonth, 0);
    const startDow = firstDay.getDay();
    const daysInMonth = lastDay.getDate();

    const holidayMap = new Map<number, Holiday[]>();
    (monthCal?.holidays ?? holidays.filter(h => {
      const d = new Date(h.date);
      return d.getMonth() + 1 === selectedMonth;
    })).forEach((h) => {
      const day = new Date(h.date).getDate();
      if (!holidayMap.has(day)) holidayMap.set(day, []);
      holidayMap.get(day)!.push(h);
    });

    const isWorkingDay = (dow: number) => {
      const config = workingDays.find((d) => d.day_of_week === dow);
      return config ? config.is_working_day : dow >= 1 && dow <= 5;
    };

    const cells = [];
    for (let i = 0; i < startDow; i++) {
      cells.push(<div key={`empty-${i}`} className="h-24 bg-gray-50 border border-gray-100" />);
    }
    for (let day = 1; day <= daysInMonth; day++) {
      const dow = (startDow + day - 1) % 7;
      const dayHolidays = holidayMap.get(day) || [];
      const isOff = !isWorkingDay(dow);
      const isToday =
        day === now.getDate() &&
        selectedMonth === now.getMonth() + 1 &&
        selectedYear === now.getFullYear();

      cells.push(
        <div
          key={day}
          className={`h-24 border border-gray-100 p-1.5 ${
            isOff ? 'bg-gray-50' : 'bg-white'
          } ${isToday ? 'ring-2 ring-black ring-inset' : ''}`}
        >
          <div className={`text-xs font-medium ${isOff ? 'text-gray-400' : 'text-gray-700'} ${isToday ? 'font-bold text-black' : ''}`}>
            {day}
          </div>
          {dayHolidays.map((h) => (
            <div
              key={h.id}
              className={`mt-0.5 text-[10px] px-1 py-0.5 rounded truncate ${
                h.holiday_type === 'public_holiday'
                  ? 'bg-red-100 text-red-700'
                  : h.holiday_type === 'company_holiday'
                  ? 'bg-blue-100 text-blue-700'
                  : h.holiday_type === 'replacement_leave'
                  ? 'bg-green-100 text-green-700'
                  : 'bg-yellow-100 text-yellow-700'
              }`}
              title={h.name}
            >
              {h.name}
            </div>
          ))}
        </div>
      );
    }

    return (
      <div className="grid grid-cols-7 gap-0">
        {DAY_INDICES.map((dow) => (
          <div key={dow} className="text-center text-xs font-medium text-gray-500 py-2 bg-gray-50 border border-gray-100">
            {weekdayName(dow)}
          </div>
        ))}
        {cells}
      </div>
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">{t('calendar.title')}</h1>
          <p className="text-sm text-gray-500 mt-1">
            {t('calendar.subtitle')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setShowImportModal(true)}
            className="px-4 py-2 border border-gray-300 text-gray-700 rounded-lg text-sm font-medium hover:bg-gray-50 flex items-center gap-2"
          >
            <Download className="w-4 h-4" />
            {t('calendar.importGoogle')}
          </button>
          <button
            onClick={() => {
              setEditingHoliday(null);
              setShowHolidayModal(true);
            }}
            className="px-4 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800"
          >
            {t('calendar.addHoliday')}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 bg-gray-100 p-1 rounded-xl w-fit max-w-full overflow-x-auto">
        {(['calendar', 'holidays', 'working-days'] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 rounded-lg text-sm font-medium whitespace-nowrap transition-all ${
              activeTab === tab
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {t(`calendar.tabs.${tab === 'working-days' ? 'workingDays' : tab}`)}
          </button>
        ))}
      </div>

      {activeTab === 'calendar' && (
        <div className="bg-white rounded-2xl border border-gray-200 p-6">
          {/* Month Navigation */}
          <div className="flex items-center justify-between mb-4">
            <button
              onClick={() => {
                if (selectedMonth === 1) {
                  setSelectedMonth(12);
                  setSelectedYear(selectedYear - 1);
                } else {
                  setSelectedMonth(selectedMonth - 1);
                }
              }}
              className="p-2 hover:bg-gray-100 rounded-lg"
            >
              &larr;
            </button>
            <div className="text-center">
              <h2 className="text-lg font-semibold">
                {monthName(selectedMonth)} {selectedYear}
              </h2>
              {monthCal && (
                <p className="text-sm text-gray-500">
                  {t('calendar.workingDaysCount', { count: monthCal.working_days })} &middot; {t('calendar.holidaysCount', { count: monthCal.holidays.length })}
                </p>
              )}
            </div>
            <button
              onClick={() => {
                if (selectedMonth === 12) {
                  setSelectedMonth(1);
                  setSelectedYear(selectedYear + 1);
                } else {
                  setSelectedMonth(selectedMonth + 1);
                }
              }}
              className="p-2 hover:bg-gray-100 rounded-lg"
            >
              &rarr;
            </button>
          </div>
          {renderCalendarGrid()}
          {/* Legend */}
          <div className="flex gap-4 mt-4 text-xs text-gray-500">
            <span className="flex items-center gap-1">
              <span className="w-3 h-3 rounded bg-red-100 border border-red-200" /> {t('enums.holidayType.public_holiday')}
            </span>
            <span className="flex items-center gap-1">
              <span className="w-3 h-3 rounded bg-blue-100 border border-blue-200" /> {t('enums.holidayType.company_holiday')}
            </span>
            <span className="flex items-center gap-1">
              <span className="w-3 h-3 rounded bg-green-100 border border-green-200" /> {t('enums.holidayType.replacement_leave')}
            </span>
            <span className="flex items-center gap-1">
              <span className="w-3 h-3 rounded bg-yellow-100 border border-yellow-200" /> {t('enums.holidayType.state_holiday')}
            </span>
            <span className="flex items-center gap-1">
              <span className="w-3 h-3 rounded bg-gray-100 border border-gray-200" /> {t('calendar.nonWorkingDay')}
            </span>
          </div>
        </div>
      )}

      {activeTab === 'holidays' && (
        <div className="bg-white rounded-2xl border border-gray-200">
          <div className="p-4 border-b border-gray-100 flex items-center gap-3">
            <label className="text-sm font-medium text-gray-700">{t('common.year')}:</label>
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
                {t('calendar.noHolidays', { year: selectedYear })}
              </div>
            )}
            {holidays.map((h) => (
              <div key={h.id} className="flex items-center justify-between px-6 py-4">
                <div className="flex items-center gap-4">
                  <div className="text-center min-w-[50px]">
                    <div className="text-xs text-gray-400 uppercase">
                      {monthName(new Date(h.date).getMonth() + 1).slice(0, 3)}
                    </div>
                    <div className="text-xl font-bold text-gray-900">
                      {new Date(h.date).getDate()}
                    </div>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-900">{h.name}</p>
                    <div className="flex items-center gap-2 mt-0.5">
                      <span
                        className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                          h.holiday_type === 'public_holiday'
                            ? 'bg-red-100 text-red-700'
                            : h.holiday_type === 'company_holiday'
                            ? 'bg-blue-100 text-blue-700'
                            : h.holiday_type === 'replacement_leave'
                            ? 'bg-green-100 text-green-700'
                            : 'bg-yellow-100 text-yellow-700'
                        }`}
                      >
                        {t(`enums.holidayType.${h.holiday_type}`, { defaultValue: h.holiday_type })}
                      </span>
                      {h.is_recurring && (
                        <span className="text-[10px] text-gray-400">{t('calendar.recurring')}</span>
                      )}
                      {h.state && (
                        <span className="text-[10px] text-gray-400">{h.state}</span>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => {
                      setEditingHoliday(h);
                      setShowHolidayModal(true);
                    }}
                    className="text-xs text-gray-500 hover:text-gray-700 px-2 py-1"
                  >
                    {t('common.edit')}
                  </button>
                  <button
                    onClick={() => {
                      if (confirm(t('calendar.deleteConfirm', { name: h.name }))) {
                        deleteMutation.mutate(h.id);
                      }
                    }}
                    className="text-xs text-red-500 hover:text-red-700 px-2 py-1"
                  >
                    {t('common.delete')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {activeTab === 'working-days' && (
        <div className="bg-white rounded-2xl border border-gray-200 p-6">
          <h3 className="text-sm font-semibold text-gray-900 mb-4">
            {t('calendar.workingDaysTitle')}
          </h3>
          <p className="text-xs text-gray-500 mb-6">
            {t('calendar.workingDaysDescription')}
          </p>
          <div className="grid grid-cols-7 gap-3">
            {DAY_INDICES.map((i) => {
              const config = workingDays.find((d) => d.day_of_week === i);
              const isWorking = config ? config.is_working_day : i >= 1 && i <= 5;
              return (
                <button
                  key={i}
                  onClick={() => toggleWorkingDay(i)}
                  className={`p-4 rounded-xl border-2 text-center transition-all ${
                    isWorking
                      ? 'border-black bg-gray-900 text-white'
                      : 'border-gray-200 bg-white text-gray-400 hover:border-gray-300'
                  }`}
                >
                  <div className="text-sm font-semibold">{weekdayName(i)}</div>
                  <div className="text-xs mt-1">{isWorking ? t('calendar.working') : t('calendar.off')}</div>
                </button>
              );
            })}
          </div>
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
      />

      {/* Google Calendar Import Modal */}
      <ImportIcsModal
        open={showImportModal}
        onClose={() => setShowImportModal(false)}
        onSuccess={() => {
          queryClient.invalidateQueries({ queryKey: ['holidays'] });
          queryClient.invalidateQueries({ queryKey: ['month-calendar'] });
        }}
      />
    </div>
  );
}

function HolidayModal({
  open,
  holiday,
  onClose,
  onSave,
  isLoading,
}: {
  open: boolean;
  holiday: Holiday | null;
  onClose: () => void;
  onSave: (data: CreateHolidayRequest) => void;
  isLoading: boolean;
}) {
  const { t } = useTranslation();
  const { register, handleSubmit, reset } = useForm<CreateHolidayRequest>({
    defaultValues: holiday
      ? {
          name: holiday.name,
          date: holiday.date,
          holiday_type: holiday.holiday_type,
          description: holiday.description || '',
          is_recurring: holiday.is_recurring,
          state: holiday.state || '',
        }
      : { holiday_type: 'public_holiday', is_recurring: false },
  });

  useEffect(() => {
    if (holiday) {
      reset({
        name: holiday.name,
        date: holiday.date,
        holiday_type: holiday.holiday_type,
        description: holiday.description || '',
        is_recurring: holiday.is_recurring,
        state: holiday.state || '',
      });
    } else {
      reset({ holiday_type: 'public_holiday', is_recurring: false, name: '', date: '', description: '', state: '' });
    }
  }, [holiday, reset]);

  return (
    <Modal open={open} onClose={onClose} title={holiday ? t('calendar.editHoliday') : t('calendar.addHoliday')}>
      <form onSubmit={handleSubmit(onSave)} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.name')}</label>
          <input
            {...register('name', { required: true })}
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
            placeholder={t('calendar.namePlaceholder')}
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.date')}</label>
          <input
            type="date"
            {...register('date', { required: true })}
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('calendar.type')}</label>
          <select
            {...register('holiday_type')}
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
          >
            {HOLIDAY_TYPE_VALUES.map((v) => (
              <option key={v} value={v}>{t(`enums.holidayType.${v}`)}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.description')}</label>
          <input
            {...register('description')}
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
            placeholder={t('teams.descriptionPlaceholder')}
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('calendar.stateOptional')}</label>
          <input
            {...register('state')}
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
            placeholder={t('calendar.statePlaceholder')}
          />
        </div>
        <div className="flex items-center gap-2">
          <input type="checkbox" {...register('is_recurring')} id="is_recurring" />
          <label htmlFor="is_recurring" className="text-sm text-gray-700">
            {t('calendar.recurringLabel')}
          </label>
        </div>
        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-700 hover:bg-gray-100 rounded-lg"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            disabled={isLoading}
            className="px-4 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-50"
          >
            {isLoading ? t('common.saving') : holiday ? t('common.update') : t('common.create')}
          </button>
        </div>
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
  const [importMode, setImportMode] = useState<'url' | 'file'>('file');
  const [url, setUrl] = useState('');
  const [file, setFile] = useState<File | null>(null);
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
    setResult(null);
    setError('');
    onClose();
  };

  return (
    <Modal open={open} onClose={handleClose} title={t('calendar.importTitle')}>
      <div className="space-y-4">
        <p className="text-sm text-gray-600">
          {t('calendar.importDescription')}
        </p>

        {/* Mode Toggle */}
        <div className="flex gap-1 bg-gray-100 p-1 rounded-xl">
          <button
            type="button"
            onClick={() => { setImportMode('file'); setError(''); }}
            className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
              importMode === 'file'
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            <Upload className="w-4 h-4" />
            {t('calendar.uploadFile')}
          </button>
          <button
            type="button"
            onClick={() => { setImportMode('url'); setError(''); }}
            className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
              importMode === 'url'
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            <LinkIcon className="w-4 h-4" />
            {t('calendar.pasteUrl')}
          </button>
        </div>

        {importMode === 'file' ? (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('calendar.icsFile')}</label>
            <label className={`flex flex-col items-center justify-center w-full h-32 border-2 border-dashed rounded-xl cursor-pointer transition-all ${
              file ? 'border-black bg-gray-50' : 'border-gray-300 hover:border-gray-400 hover:bg-gray-50'
            }`}>
              <div className="flex flex-col items-center justify-center pt-5 pb-6">
                <Upload className={`w-8 h-8 mb-2 ${file ? 'text-black' : 'text-gray-400'}`} />
                {file ? (
                  <>
                    <p className="text-sm font-medium text-gray-900">{file.name}</p>
                    <p className="text-xs text-gray-500 mt-0.5">{(file.size / 1024).toFixed(1)} KB</p>
                  </>
                ) : (
                  <>
                    {/* i18n-ok: `.ics` is a file extension, not copy */}
                    <p className="text-sm text-gray-500">{t('calendar.clickToSelect')} <span className="font-medium">.ics</span></p>
                    <p className="text-xs text-gray-400 mt-0.5">{t('calendar.orDragDrop')}</p>
                  </>
                )}
              </div>
              <input
                type="file"
                accept=".ics,.ical"
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) setFile(f);
                }}
              />
            </label>
            <p className="text-xs text-gray-400 mt-2">
              {t('calendar.icsExportHint')}
            </p>
          </div>
        ) : (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('calendar.icsUrl')}</label>
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm"
              placeholder="https://calendar.google.com/calendar/ical/..."
            />
            <div className="bg-gray-50 border border-gray-200 rounded-lg p-3 mt-2 text-xs text-gray-500 space-y-1">
              <p className="font-medium text-gray-700">{t('calendar.icsHowTo')}</p>
              <ol className="list-decimal list-inside space-y-0.5">
                <li>{t('calendar.icsStep1')}</li>
                <li>{t('calendar.icsStep2')}</li>
                <li>{t('calendar.icsStep3')}</li>
              </ol>
            </div>
          </div>
        )}

        {error && (
          <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">
            {error}
          </div>
        )}

        {result && (
          <div className="p-3 bg-emerald-50 text-emerald-700 text-sm rounded-lg border border-emerald-100">
            {t('calendar.importSuccess', { count: result.count })}
          </div>
        )}

        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            onClick={handleClose}
            className="px-4 py-2 text-sm text-gray-700 hover:bg-gray-100 rounded-lg"
          >
            {result ? t('common.done') : t('common.cancel')}
          </button>
          {!result && (
            <button
              onClick={handleImport}
              disabled={isPending}
              className="px-4 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-50 flex items-center gap-2"
            >
              <Download className="w-4 h-4" />
              {isPending ? t('common.importing') : t('calendar.importHolidays')}
            </button>
          )}
        </div>
      </div>
    </Modal>
  );
}
