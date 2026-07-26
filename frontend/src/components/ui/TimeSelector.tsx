import { Clock3 } from 'lucide-react';

interface TimeSelectorProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  minuteStep?: number;
  disabled?: boolean;
}

function pad(value: number) {
  return String(value).padStart(2, '0');
}

function getMinuteOptions(step: number) {
  const minutes: number[] = [];
  for (let minute = 0; minute < 60; minute += step) {
    minutes.push(minute);
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
}: TimeSelectorProps) {
  const { hour, minute } = getValueParts(value || '');
  const minuteOptions = getMinuteOptions(minuteStep);

  const setHour = (nextHour: string) => onChange(`${nextHour}:${minute}`);
  const setMinute = (nextMinute: string) => onChange(`${hour}:${nextMinute}`);

  // Native select chrome (the chevron) collides with the value once the control
  // gets narrow — two of these sit side by side inside a card. `appearance-none`
  // plus a centred value keeps "09" readable at phone widths.
  const selectClass =
    'w-12 shrink-0 appearance-none bg-transparent py-1.5 text-center text-sm font-semibold ' +
    'tabular-nums text-gray-900 outline-none disabled:text-gray-400 disabled:cursor-not-allowed';

  return (
    <div>
      <label className="form-label">{label}</label>
      <div
        className={`flex items-center gap-1 rounded-xl border border-gray-300 bg-white px-2.5 py-1.5 transition-shadow focus-within:border-gray-900 focus-within:ring-1 focus-within:ring-gray-900 ${
          disabled ? 'bg-gray-50' : ''
        }`}
      >
        <Clock3 className="w-4 h-4 shrink-0 text-gray-400" />
        <select
          value={hour}
          onChange={(event) => setHour(event.target.value)}
          className={selectClass}
          disabled={disabled}
          aria-label={`${label} — hour`}
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
        <span className="text-sm font-semibold text-gray-300">:</span>
        <select
          value={minute}
          onChange={(event) => setMinute(event.target.value)}
          className={selectClass}
          disabled={disabled}
          aria-label={`${label} — minute`}
        >
          {minuteOptions.map((option) => {
            const value = pad(option);
            return (
              <option key={value} value={value}>
                {value}
              </option>
            );
          })}
        </select>
      </div>
    </div>
  );
}
