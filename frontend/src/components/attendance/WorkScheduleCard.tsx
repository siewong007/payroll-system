import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Clock, CheckCircle2, AlertCircle } from 'lucide-react';
import { getDefaultSchedule, upsertDefaultSchedule } from '@/api/workSchedule';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { Trans } from 'react-i18next';

const TIMEZONES = [
  'Asia/Kuala_Lumpur',
  'Asia/Singapore',
  'Asia/Jakarta',
  'Asia/Bangkok',
  'Asia/Manila',
  'Asia/Hong_Kong',
  'Asia/Tokyo',
  'Asia/Seoul',
  'Asia/Kolkata',
  'Asia/Dubai',
  'Australia/Sydney',
  'Europe/London',
  'America/New_York',
  'America/Los_Angeles',
];

function timeToHHMM(t: string | undefined): string {
  if (!t) return '09:00';
  // Handle "HH:MM:SS" → "HH:MM"
  return t.slice(0, 5);
}

export function WorkScheduleCard() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [toast, setToast] = useState('');
  const [toastType, setToastType] = useState<'success' | 'error'>('success');

  const { data, isLoading } = useQuery({
    queryKey: ['work-schedule-default'],
    queryFn: getDefaultSchedule,
  });

  const schedule = data?.schedule;

  const [form, setForm] = useState({
    start_time: '09:00',
    end_time: '18:00',
    grace_minutes: 15,
    timezone: 'Asia/Kuala_Lumpur',
  });

  // Sync from server once
  const [synced, setSynced] = useState(false);
  if (schedule && !synced) {
    setForm({
      start_time: timeToHHMM(schedule.start_time),
      end_time: timeToHHMM(schedule.end_time),
      grace_minutes: schedule.grace_minutes,
      timezone: schedule.timezone,
    });
    setSynced(true);
  }

  const mutation = useMutation({
    mutationFn: () => upsertDefaultSchedule({
      start_time: form.start_time,
      end_time: form.end_time,
      grace_minutes: form.grace_minutes,
      timezone: form.timezone,
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['work-schedule-default'] });
      setToast(t('attendance.schedule.saved'));
      setToastType('success');
      setTimeout(() => setToast(''), 3000);
    },
    onError: () => {
      setToast(t('attendance.schedule.saveFailed'));
      setToastType('error');
      setTimeout(() => setToast(''), 3000);
    },
  });

  if (isLoading) {
    return (
      <div className="bg-white rounded-2xl shadow p-6 flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  return (
    <div className="bg-white rounded-2xl shadow">
      <div className="p-5 sm:p-6 border-b border-gray-100">
        <div className="flex items-center gap-2 mb-1">
          <Clock className="w-5 h-5 text-gray-700" />
          <h2 className="font-semibold text-gray-900">{t('attendance.schedule.title')}</h2>
        </div>
        <p className="text-sm text-gray-500">
          <Trans i18nKey="attendance.schedule.body" components={{ strong: <strong /> }} />
        </p>
      </div>

      <div className="p-5 sm:p-6 space-y-5">
        {/* Time row — stacked on phones; two 150px-wide time controls side by
            side left no room for the values. */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <TimeSelector
            label={t('attendance.schedule.startTime')}
            value={form.start_time}
            onChange={(value) => setForm(p => ({ ...p, start_time: value }))}
            minuteStep={15}
          />
          <TimeSelector
            label={t('attendance.schedule.endTime')}
            value={form.end_time}
            onChange={(value) => setForm(p => ({ ...p, end_time: value }))}
            minuteStep={15}
          />
        </div>

        {/* Grace period */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            {t('attendance.schedule.gracePeriod')}
          </label>
          <input
            type="number"
            min={0}
            max={120}
            value={form.grace_minutes}
            onChange={e => setForm(p => ({ ...p, grace_minutes: parseInt(e.target.value) || 0 }))}
            className="w-32 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
          />
          <p className="text-xs text-gray-400 mt-1">
            {t('attendance.schedule.graceHint', { minutes: form.grace_minutes, start: form.start_time })}
          </p>
        </div>

        {/* Timezone */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.schedule.timezone')}</label>
          <select
            value={form.timezone}
            onChange={e => setForm(p => ({ ...p, timezone: e.target.value }))}
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
          >
            {TIMEZONES.map(tz => (
              <option key={tz} value={tz}>{tz.replace('_', ' ')}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Save */}
      <div className="flex items-center justify-between gap-3 px-5 sm:px-6 py-4 border-t border-gray-100 bg-gray-50 rounded-b-2xl">
        <div className="min-w-0 h-5">
          {toast && (
            <span className={`flex items-center gap-1.5 text-sm font-medium ${
              toastType === 'success' ? 'text-emerald-600' : 'text-red-600'
            }`}>
              {toastType === 'success' ? <CheckCircle2 className="w-4 h-4 shrink-0" /> : <AlertCircle className="w-4 h-4 shrink-0" />}
              <span className="truncate">{toast}</span>
            </span>
          )}
        </div>
        <button
          onClick={() => mutation.mutate()}
          disabled={mutation.isPending}
          className="shrink-0 whitespace-nowrap bg-black text-white px-5 py-2 rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
        >
          {mutation.isPending ? t('common.saving') : t('attendance.schedule.save')}
        </button>
      </div>
    </div>
  );
}
