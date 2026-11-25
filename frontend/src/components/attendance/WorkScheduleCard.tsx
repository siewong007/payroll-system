import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Clock, CheckCircle2, AlertCircle, RefreshCw } from 'lucide-react';
import { getDefaultSchedule, upsertDefaultSchedule } from '@/api/workSchedule';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { FormField } from '@/components/ui/FormField';
import { getErrorMessage } from '@/lib/utils';

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

  const { data, isLoading, isError, error: loadError, refetch } = useQuery({
    queryKey: ['work-schedule-default'],
    queryFn: getDefaultSchedule,
  });

  const schedule = data?.schedule;
  const scheduleKey = schedule
    ? `${schedule.company_id}:${schedule.id}:${schedule.updated_at}`
    : 'default';

  const [drafts, setDrafts] = useState<{
    key: string;
    start_time?: string;
    end_time?: string;
    grace_minutes?: number;
    timezone?: string;
  }>({ key: 'default' });

  const activeDraft = drafts.key === scheduleKey ? drafts : { key: scheduleKey };
  const form = {
    start_time: activeDraft.start_time ?? timeToHHMM(schedule?.start_time),
    end_time: activeDraft.end_time ?? (schedule ? timeToHHMM(schedule.end_time) : '18:00'),
    grace_minutes: activeDraft.grace_minutes ?? schedule?.grace_minutes ?? 15,
    timezone: activeDraft.timezone ?? schedule?.timezone ?? 'Asia/Kuala_Lumpur',
  };

  const timezoneOptions =
    form.timezone && !TIMEZONES.includes(form.timezone) ? [form.timezone, ...TIMEZONES] : TIMEZONES;

  const updateForm = (patch: Partial<Omit<typeof drafts, 'key'>>) => {
    setDrafts({ ...activeDraft, key: scheduleKey, ...patch });
  };

  const mutation = useMutation({
    mutationFn: () => upsertDefaultSchedule({
      start_time: form.start_time,
      end_time: form.end_time,
      grace_minutes: form.grace_minutes,
      timezone: form.timezone,
    }),
    onSuccess: () => {
      setDrafts({ key: 'default' });
      void queryClient.invalidateQueries({ queryKey: ['work-schedule-default'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      setToast(t('attendance.schedule.saved'));
      setToastType('success');
      setTimeout(() => setToast(''), 3000);
    },
    onError: (err) => {
      setToast(getErrorMessage(err, t('attendance.schedule.saveFailed')));
      setToastType('error');
      setTimeout(() => setToast(''), 5000);
    },
  });

  if (isLoading) {
    return (
      <div className="card p-6 flex items-center justify-center h-48">
        <div className="spinner" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="card p-6 flex flex-col items-center justify-center gap-3 h-48 text-center">
        <AlertCircle className="w-8 h-8 text-red-500" />
        <p className="text-sm text-gray-600 dark:text-slate-300">
          {getErrorMessage(loadError, t('attendance.schedule.loadFailed'))}
        </p>
        <button type="button" onClick={() => void refetch()} className="btn-secondary !py-1.5">
          <RefreshCw className="w-3.5 h-3.5" />
          {t('common.retry')}
        </button>
      </div>
    );
  }

  return (
    <div className="card !p-0 overflow-hidden">
      <div className="p-5 sm:p-6 border-b border-gray-100 dark:border-slate-800">
        <div className="flex items-center gap-2 mb-1">
          <Clock className="w-5 h-5 text-[var(--accent-1)]" />
          <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('attendance.schedule.title')}</h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-slate-400">
          <Trans i18nKey="attendance.schedule.body" components={{ strong: <strong /> }} />
        </p>
      </div>

      <div className="p-5 sm:p-6 space-y-5">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <TimeSelector
            label={t('attendance.schedule.startTime')}
            value={form.start_time}
            onChange={(value) => updateForm({ start_time: value })}
            minuteStep={15}
          />
          <TimeSelector
            label={t('attendance.schedule.endTime')}
            value={form.end_time}
            onChange={(value) => updateForm({ end_time: value })}
            minuteStep={15}
          />
        </div>

        {/* Grace period */}
        <FormField
          label={t('attendance.schedule.gracePeriod')}
          hint={t('attendance.schedule.graceHint', { minutes: form.grace_minutes, start: form.start_time })}
        >
          {(aria) => (
            <input
              {...aria}
              type="number"
              min={0}
              max={120}
              value={form.grace_minutes}
              onChange={e => updateForm({ grace_minutes: Math.max(0, Math.min(120, parseInt(e.target.value, 10) || 0)) })}
              className="form-input !w-32"
            />
          )}
        </FormField>

        {/* Timezone */}
        <FormField label={t('attendance.schedule.timezone')}>
          {(aria) => (
            <select
              {...aria}
              value={form.timezone}
              onChange={e => updateForm({ timezone: e.target.value })}
              className="form-input"
            >
              {timezoneOptions.map(tz => (
                <option key={tz} value={tz}>{tz.replace('_', ' ')}</option>
              ))}
            </select>
          )}
        </FormField>
      </div>

      {/* Save */}
      <div className="flex items-center justify-between gap-3 px-5 sm:px-6 py-4 border-t border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-950/50">
        <div className="min-w-0 min-h-5">
          {toast && (
            <span
              role={toastType === 'error' ? 'alert' : 'status'}
              className={`flex items-center gap-1.5 text-sm font-medium ${
                toastType === 'success' ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'
              }`}
            >
              {toastType === 'success' ? <CheckCircle2 className="w-4 h-4 shrink-0" /> : <AlertCircle className="w-4 h-4 shrink-0" />}
              <span className="truncate">{toast}</span>
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={() => mutation.mutate()}
          disabled={mutation.isPending}
          className="btn-primary shrink-0 whitespace-nowrap"
        >
          {mutation.isPending ? t('common.saving') : t('attendance.schedule.save')}
        </button>
      </div>
    </div>
  );
}
