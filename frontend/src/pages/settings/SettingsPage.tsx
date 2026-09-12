import { useMemo, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Bell,
  Calculator,
  Check,
  Landmark,
  MonitorSmartphone,
  ShieldCheck,
  SlidersHorizontal,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { getSettings, bulkUpdateSettings } from '@/api/settings';
import type { CompanySetting, SettingUpdate } from '@/types';
import { SessionManagement } from '@/components/SessionManagement';
import { SettingsNav, type SettingsNavItem } from './SettingsNav';
import { CompanyCategorySection } from './CompanyCategorySection';
import { SecuritySection } from './SecuritySection';

const SECTION_META: Record<string, { label: string; icon: LucideIcon; description: string }> = {
  system: {
    label: 'General',
    icon: SlidersHorizontal,
    description: 'Core workspace preferences',
  },
  payroll: {
    label: 'Payroll',
    icon: Calculator,
    description: 'Payroll calculation and payslip defaults',
  },
  statutory: {
    label: 'Statutory',
    icon: Landmark,
    description: 'Statutory contribution behavior',
  },
  notifications: {
    label: 'Notifications',
    icon: Bell,
    description: 'Workspace notification defaults',
  },
  security: {
    label: 'Security',
    icon: ShieldCheck,
    description: 'Your sign-in credentials and account protection',
  },
  sessions: {
    label: 'Sessions',
    icon: MonitorSmartphone,
    description: 'Devices signed in to your account',
  },
};

const WORKSPACE_ORDER = ['system', 'payroll', 'statutory', 'notifications'];
const ACCOUNT_SECTIONS = ['security', 'sessions'];

export function SettingsPage() {
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saved, setSaved] = useState(false);

  const { data: settings, isLoading } = useQuery({
    queryKey: ['settings'],
    queryFn: () => getSettings(),
  });

  const grouped = useMemo(() => {
    const map: Record<string, CompanySetting[]> = {};
    settings?.forEach((s) => {
      (map[s.category] ??= []).push(s);
    });
    return map;
  }, [settings]);

  const sections = useMemo<SettingsNavItem[]>(
    () => [
      ...WORKSPACE_ORDER.filter((c) => grouped[c]?.length).map((c) => ({
        id: c,
        label: SECTION_META[c].label,
        icon: SECTION_META[c].icon,
        group: 'Workspace' as const,
      })),
      ...ACCOUNT_SECTIONS.map((id) => ({
        id,
        label: SECTION_META[id].label,
        icon: SECTION_META[id].icon,
        group: 'Account' as const,
      })),
    ],
    [grouped],
  );

  const requested = searchParams.get('section');
  const active = sections.some((s) => s.id === requested)
    ? requested!
    : (sections[0]?.id ?? 'security');

  const mutation = useMutation({
    mutationFn: (updates: SettingUpdate[]) => bulkUpdateSettings(updates),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] });
      setEdits({});
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const activeEdits = () =>
    Object.entries(edits)
      .filter(([key]) => key.startsWith(active + '/'))
      .map(([key, value]) => {
        const [category, settingKey] = key.split('/', 2);
        return { category, key: settingKey, value };
      });

  const handleSave = () => {
    const updates = activeEdits();
    if (updates.length > 0) mutation.mutate(updates);
  };

  const handleDiscard = () =>
    setEdits((prev) =>
      Object.fromEntries(Object.entries(prev).filter(([k]) => !k.startsWith(active + '/'))),
    );

  const isWorkspace = WORKSPACE_ORDER.includes(active);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="spinner" />
      </div>
    );
  }

  return (
    <div>
      <div className="page-header flex items-center justify-between mb-6">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">Manage workspace and account preferences</p>
        </div>
        {saved && (
          <span className="flex items-center gap-1 text-sm text-green-600 font-medium">
            <Check className="w-4 h-4" /> Saved
          </span>
        )}
      </div>

      <div className="lg:grid lg:grid-cols-[220px_1fr] lg:gap-6 lg:items-start">
        <SettingsNav
          sections={sections}
          active={active}
          onSelect={(id) => setSearchParams({ section: id })}
        />

        <div key={active} className="mt-4 lg:mt-0 animate-fade-up">
          {isWorkspace ? (
            <CompanyCategorySection
              title={SECTION_META[active].label}
              description={SECTION_META[active].description}
              settings={grouped[active] ?? []}
              edits={edits}
              onEdit={(s, v) => setEdits((prev) => ({ ...prev, [`${s.category}/${s.key}`]: v }))}
              onSave={handleSave}
              onDiscard={handleDiscard}
              saving={mutation.isPending}
            />
          ) : (
            <>
              <h2 className="section-title">{SECTION_META[active].label}</h2>
              <p className="text-sm text-gray-500 mt-1 mb-4">{SECTION_META[active].description}</p>
              {active === 'security' ? <SecuritySection /> : <SessionManagement />}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
