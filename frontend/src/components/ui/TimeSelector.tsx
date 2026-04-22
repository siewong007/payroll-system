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
  const [hour = '09', minute = '00'] = value.split(':');
  return { hour, minute };
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

  return (
    <div>
      <label className="form-label">{label}</label>
      <div className="rounded-xl border border-gray-300 bg-white px-3 py-3">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-gray-400">
          <Clock3 className="w-3.5 h-3.5" />
          Time Selector
        </div>
        <div className="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2">
          <select
            value={hour}
            onChange={(event) => setHour(event.target.value)}
            className="form-input !mb-0 !py-2"
            disabled={disabled}
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
          <span className="text-lg font-semibold text-gray-400">:</span>
          <select
            value={minute}
            onChange={(event) => setMinute(event.target.value)}
            className="form-input !mb-0 !py-2"
            disabled={disabled}
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
        <p className="mt-2 text-xs text-gray-500">
          Selected time: <span className="font-medium text-gray-700">{hour}:{minute}</span>
        </p>
      </div>
    </div>
  );
}
