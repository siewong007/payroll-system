import { Loader2, Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { CompanySetting } from '@/types';

export function CompanyCategorySection({
  title,
  description,
  settings,
  edits,
  onEdit,
  onSave,
  onDiscard,
  saving,
}: {
  title: string;
  description: string;
  settings: CompanySetting[];
  edits: Record<string, unknown>;
  onEdit: (setting: CompanySetting, value: unknown) => void;
  onSave: () => void;
  onDiscard: () => void;
  saving: boolean;
}) {
  const { t } = useTranslation();
  const dirtyCount = settings.filter((s) => `${s.category}/${s.key}` in edits).length;

  return (
    <div className="card !p-0 overflow-x-clip">
      <div className="p-6 pb-0">
        <h2 className="section-title">{title}</h2>
        <p className="text-sm text-gray-600 dark:text-slate-300 mt-1 mb-4">{description}</p>
      </div>
      <div className="px-6 divide-y divide-gray-100 dark:divide-slate-800 [&>*]:py-4">
        {settings.map((s) => (
          <SettingField
            key={s.key}
            setting={s}
            value={`${s.category}/${s.key}` in edits ? edits[`${s.category}/${s.key}`] : s.value}
            onChange={(v) => onEdit(s, v)}
          />
        ))}
      </div>
      {dirtyCount > 0 && (
        <div className="sticky bottom-0 z-10 flex flex-wrap items-center justify-between gap-3 px-6 py-3.5 rounded-b-xl border-t border-[var(--border)] bg-[var(--card)]">
          <span className="text-sm text-gray-600 dark:text-slate-300">
            {t('settings.unsavedChanges', { count: dirtyCount })}
          </span>
          <div className="flex w-full sm:w-auto gap-2">
            <button
              type="button"
              disabled={saving}
              className="btn-secondary flex-1 sm:flex-initial min-h-11 sm:min-h-9 !py-2"
              onClick={onDiscard}
            >
              {t('common.discard')}
            </button>
            <button
              type="button"
              className="btn-primary flex-1 sm:flex-initial min-h-11 sm:min-h-9 !py-2 inline-flex items-center justify-center gap-2"
              onClick={onSave}
              disabled={saving}
            >
              {saving ? (
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
              ) : (
                <Save className="w-4 h-4" aria-hidden="true" />
              )}
              {saving ? t('common.saving') : t('settings.saveChanges')}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function SettingField({
  setting,
  value,
  onChange,
}: {
  setting: CompanySetting;
  value: unknown;
  onChange: (val: unknown) => void;
}) {
  const { t } = useTranslation();
  const inputId = `setting-${setting.category}-${setting.key}`;
  const descId = setting.description ? `${inputId}-desc` : undefined;
  const isBool = typeof setting.value === 'boolean';
  const isNumber =
    typeof setting.value === 'number' ||
    (typeof setting.value === 'string' &&
      setting.value.trim() !== '' &&
      Number.isFinite(Number(setting.value)));
  const fieldClass =
    'px-3 py-2 min-h-11 sm:min-h-10 border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-slate-100 rounded-lg text-base sm:text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]';

  // Rest time: show with "minutes" suffix
  if (setting.key === 'rest_time_minutes') {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <input
            id={inputId}
            aria-describedby={descId}
            type="number"
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            min="0"
            max="180"
            step="15"
            className={`${fieldClass} text-right w-24`}
          />
          <span className="text-sm text-gray-600 dark:text-slate-300">{t('settings.units.min')}</span>
        </div>
      </div>
    );
  }

  // Effective hours: show as read-hint
  if (setting.key === 'effective_hours_per_day') {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <input
            id={inputId}
            aria-describedby={descId}
            type="number"
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            min="1"
            max="24"
            step="0.5"
            className={`${fieldClass} text-right w-24`}
          />
          <span className="text-sm text-gray-600 dark:text-slate-300">{t('settings.units.hrs')}</span>
        </div>
      </div>
    );
  }

  // Special handling for rounding_method
  if (setting.key === 'rounding_method') {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <select
          id={inputId}
          aria-describedby={descId}
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          className={`${fieldClass} w-full sm:w-44`}
        >
          <option value="nearest">{t('settings.rounding.nearest')}</option>
          <option value="up">{t('settings.rounding.up')}</option>
          <option value="down">{t('settings.rounding.down')}</option>
        </select>
      </div>
    );
  }

  // Special handling for payslip_template
  if (setting.key === 'payslip_template') {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <select
          id={inputId}
          aria-describedby={descId}
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          className={`${fieldClass} w-full sm:w-44`}
        >
          <option value="default">{t('common.default')}</option>
          <option value="detailed">{t('settings.template.detailed')}</option>
          <option value="compact">{t('settings.template.compact')}</option>
        </select>
      </div>
    );
  }

  if (isBool) {
    return (
      <div className="flex items-center justify-between gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <button
          id={inputId}
          type="button"
          role="switch"
          aria-checked={Boolean(value)}
          aria-describedby={descId}
          onClick={() => onChange(!value)}
          className="relative inline-flex min-h-11 min-w-12 shrink-0 items-center justify-end rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
        >
          <span
            aria-hidden="true"
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              value ? 'bg-[var(--accent-1)]' : 'bg-zinc-300 dark:bg-zinc-700'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                value ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </span>
        </button>
      </div>
    );
  }

  if (isNumber && !setting.key.includes('format') && !setting.key.includes('currency')) {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div>
          <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
          {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
        </div>
        <input
          id={inputId}
          aria-describedby={descId}
          type="number"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          step="any"
          className={`${fieldClass} text-right w-32`}
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
      <div>
        <label htmlFor={inputId} className="text-sm font-medium text-gray-900 dark:text-slate-100">{setting.label || setting.key}</label>
        {setting.description && <p id={descId} className="text-xs text-gray-600 dark:text-slate-400 mt-0.5">{setting.description}</p>}
      </div>
      <input
        id={inputId}
        aria-describedby={descId}
        type="text"
        value={String(value ?? '')}
        onChange={(e) => onChange(e.target.value)}
        className={`${fieldClass} w-full sm:w-52`}
      />
    </div>
  );
}
