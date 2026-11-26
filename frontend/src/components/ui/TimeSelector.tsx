import { Clock3 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface TimeSelectorProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  minuteStep?: number;
  disabled?: boolean;
  required?: boolean;
}

function pad(value: number) {
  return String(value).padStart(2, '0');
}

function getMinuteOptions(step: number, currentMinute: string) {
  const minutes: number[] = [];
  for (let minute = 0; minute < 60; minute += step) {
    minutes.push(minute);
  }
  const parsed = Number(currentMinute);
  if (Number.isInteger(parsed) && parsed >= 0 && parsed < 60 && !minutes.includes(parsed)) {
    minutes.push(parsed);
    minutes.sort((a, b) => a - b);
  }
  return minutes;
}

function getValueParts(value: string) {
  const [hour, minute] = value.split(':');
  return { hour: hour || '09', minute: minute || '00' };
}

export function TimeSelector({
  label,
  value,
  onChange,
  minuteStep = 30,
  disabled = false,
  required = false,
}: TimeSelectorProps) {
  const { t } = useTranslation();
  const { hour, minute } = getValueParts(value || '');
  const minuteOptions = getMinuteOptions(minuteStep, minute);

  const setHour = (nextHour: string) => onChange(`${nextHour}:${minute}`);
  const setMinute = (nextMinute: string) => onChange(`${hour}:${nextMinute}`);

  // Native select chrome (the chevron) collides with the value once the control
  // gets narrow — two of these sit side by side inside a card. `appearance-none`
  // plus a centred value keeps "09" readable at phone widths.
  const selectClass =
    'flex-1 min-w-0 appearance-none bg-transparent py-1.5 text-center text-base sm:text-sm font-semibold ' +
    'tabular-nums text-gray-900 dark:text-slate-100 outline-none disabled:text-gray-400 dark:disabled:text-slate-500 disabled:cursor-not-allowed [&>option]:bg-white dark:[&>option]:bg-slate-900';

  return (
    <fieldset className="border-0 p-0 m-0 min-w-0">
      <legend className="form-label p-0">
        {label}
        {required && <span aria-hidden="true"> *</span>}
        {required && <span className="sr-only"> ({t('common.required')})</span>}
      </legend>
      <div
        className={`flex items-center gap-1 rounded-xl border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-900/80 px-2.5 py-1.5 transition-shadow focus-within:border-[var(--ring)] focus-within:ring-2 focus-within:ring-[var(--ring)]/25 ${
          disabled ? 'bg-gray-50 dark:bg-slate-800/60' : ''
        }`}
      >
        <Clock3 className="w-4 h-4 shrink-0 text-gray-400 dark:text-slate-400" aria-hidden="true" />
        <select
          value={hour}
          onChange={(event) => setHour(event.target.value)}
          className={selectClass}
          disabled={disabled}
          aria-label={`${label} — ${t('a11y.hour')}`}
        >
          {Array.from({ length: 24 }, (_, index) => {
            const option = pad(index);
            return (
              <option key={option} value={option}>
                {option}
              </option>
            );
          })}
        </select>
        <span className="text-sm font-semibold text-gray-300 dark:text-slate-600">:</span>
        <select
          value={minute}
          onChange={(event) => setMinute(event.target.value)}
          className={selectClass}
          disabled={disabled}
          aria-label={`${label} — ${t('a11y.minute')}`}
        >
          {minuteOptions.map((option) => {
            const optVal = pad(option);
            return (
              <option key={optVal} value={optVal}>
                {optVal}
              </option>
            );
          })}
        </select>
      </div>
    </fieldset>
  );
}
