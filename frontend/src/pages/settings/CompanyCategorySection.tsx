import { Save } from 'lucide-react';
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
    <div className="card !p-0 overflow-hidden">
      <div className="p-6 pb-0">
        <h2 className="section-title">{title}</h2>
        <p className="text-sm text-gray-500 mt-1 mb-4">{description}</p>
      </div>
      <div className="px-6 divide-y divide-gray-100 [&>*]:py-4">
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
        <div className="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-3.5 border-t border-gray-200/80 bg-white/85 backdrop-blur">
          <span className="text-sm text-gray-500">
            {t('settings.unsavedChanges', { count: dirtyCount })}
          </span>
          <div className="flex gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={onDiscard}>
              {t('common.discard')}
            </button>
            <button
              type="button"
              className="btn-primary !min-h-0 !py-2"
              onClick={onSave}
              disabled={saving}
            >
              <Save className="w-4 h-4" /> {saving ? t('common.saving') : t('settings.saveChanges')}
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
  const isBool = typeof setting.value === 'boolean';
  const isNumber = typeof setting.value === 'number' || (typeof setting.value === 'string' && !isNaN(Number(setting.value)));

  // Rest time: show with "minutes" suffix
  if (setting.key === 'rest_time_minutes') {
    return (
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <input
            type="number"
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            min="0"
            max="180"
            step="15"
            className="px-3 py-2 border border-gray-300 rounded-lg text-sm text-right focus:ring-1 focus:ring-black outline-none w-24"
          />
          <span className="text-sm text-gray-500">{t('settings.units.min')}</span>
        </div>
      </div>
    );
  }

  // Effective hours: show as read-hint
  if (setting.key === 'effective_hours_per_day') {
    return (
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <input
            type="number"
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            min="1"
            max="24"
            step="0.5"
            className="px-3 py-2 border border-gray-300 rounded-lg text-sm text-right focus:ring-1 focus:ring-black outline-none w-24"
          />
          <span className="text-sm text-gray-500">{t('settings.units.hrs')}</span>
        </div>
      </div>
    );
  }

  // Special handling for rounding_method
  if (setting.key === 'rounding_method') {
    return (
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <select
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none w-40"
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
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <select
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none w-40"
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
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <button
          type="button"
          onClick={() => onChange(!value)}
          className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
            value ? 'bg-black' : 'bg-gray-300'
          }`}
        >
          <span
            className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
              value ? 'translate-x-6' : 'translate-x-1'
            }`}
          />
        </button>
      </div>
    );
  }

  if (isNumber && !setting.key.includes('format') && !setting.key.includes('currency')) {
    return (
      <div className="flex items-center justify-between">
        <div>
          <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
          {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
        </div>
        <input
          type="number"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          step="any"
          className="px-3 py-2 border border-gray-300 rounded-lg text-sm text-right focus:ring-1 focus:ring-black outline-none w-32"
        />
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between">
      <div>
        <label className="text-sm font-medium text-gray-900">{setting.label || setting.key}</label>
        {setting.description && <p className="text-xs text-gray-500 mt-0.5">{setting.description}</p>}
      </div>
      <input
        type="text"
        value={String(value ?? '')}
        onChange={(e) => onChange(e.target.value)}
        className="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none w-48"
      />
    </div>
  );
}
