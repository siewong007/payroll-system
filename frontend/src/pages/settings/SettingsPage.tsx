import { useState, useMemo } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Save, Check } from 'lucide-react';
import { getSettings, bulkUpdateSettings } from '@/api/settings';
import type { CompanySetting, SettingUpdate } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { PasskeyManagement } from '@/components/PasskeyManagement';

const CATEGORY_LABELS: Record<string, string> = {
  payroll: 'Payroll',
  statutory: 'Statutory',
  system: 'System',
  notifications: 'Notifications',
};

const CATEGORY_ORDER = ['payroll', 'statutory', 'system', 'notifications'];

export function SettingsPage() {
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState(user?.role === 'exec' ? 'system' : 'payroll');
  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saved, setSaved] = useState(false);

  const { data: settings, isLoading } = useQuery({
    queryKey: ['settings'],
    queryFn: () => getSettings(),
  });

  const grouped = useMemo(() => {
    const map: Record<string, CompanySetting[]> = {};
    settings?.forEach(s => {
      if (!map[s.category]) map[s.category] = [];
      map[s.category].push(s);
    });
    return map;
  }, [settings]);

  const categories = useMemo(
    () => CATEGORY_ORDER.filter(c => grouped[c]?.length),
    [grouped],
  );

  const mutation = useMutation({
    mutationFn: (updates: SettingUpdate[]) => bulkUpdateSettings(updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] });
      setEdits({});
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const handleSave = () => {
    const updates: SettingUpdate[] = Object.entries(edits)
      .filter(([key]) => key.startsWith(activeTab + '/'))
      .map(([key, value]) => {
        const [category, settingKey] = key.split('/', 2);
        return { category, key: settingKey, value };
      });
    if (updates.length > 0) {
      mutation.mutate(updates);
    }
  };

  const getEditKey = (s: CompanySetting) => `${s.category}/${s.key}`;

  const getValue = (s: CompanySetting) => {
    const editKey = getEditKey(s);
    return editKey in edits ? edits[editKey] : s.value;
  };

  const setValue = (s: CompanySetting, value: unknown) => {
    setEdits(prev => ({ ...prev, [getEditKey(s)]: value }));
  };

  const hasChanges = Object.keys(edits).some(k => k.startsWith(activeTab + '/'));

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-gray-900">Settings</h1>
        {saved && (
          <span className="flex items-center gap-1 text-sm text-green-600 font-medium">
            <Check className="w-4 h-4" /> Saved
          </span>
        )}
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 bg-gray-100 p-1 rounded-lg w-fit max-w-full overflow-x-auto">
        {categories.map(cat => (
          <button
            key={cat}
            onClick={() => setActiveTab(cat)}
            className={`px-4 py-2 text-sm font-medium rounded-md whitespace-nowrap transition-colors ${
              activeTab === cat
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {CATEGORY_LABELS[cat] || cat}
          </button>
        ))}
      </div>

      {/* Settings Form */}
      <div className="bg-white rounded-2xl shadow">
        <div className="p-6 space-y-6">
          {grouped[activeTab]?.map(setting => (
            <SettingField
              key={setting.key}
              setting={setting}
              value={getValue(setting)}
              onChange={(val) => setValue(setting, val)}
            />
          ))}
        </div>

        {/* Save Bar */}
        <div className="flex justify-end px-6 py-4 border-t border-gray-200 bg-gray-50 rounded-b-2xl">
          <button
            onClick={handleSave}
            disabled={!hasChanges || mutation.isPending}
            className="flex items-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Save className="w-4 h-4" />
            {mutation.isPending ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>

      {/* Passkey Management */}
      <div className="mt-6">
        <PasskeyManagement />
      </div>
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
          <span className="text-sm text-gray-500">min</span>
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
          <span className="text-sm text-gray-500">hrs</span>
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
          <option value="nearest">Nearest</option>
          <option value="up">Round Up</option>
          <option value="down">Round Down</option>
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
          <option value="default">Default</option>
          <option value="detailed">Detailed</option>
          <option value="compact">Compact</option>
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
