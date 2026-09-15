import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
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

const SECTION_ICONS: Record<string, LucideIcon> = {
  system: SlidersHorizontal,
  payroll: Calculator,
  statutory: Landmark,
  notifications: Bell,
  security: ShieldCheck,
  sessions: MonitorSmartphone,
};

const WORKSPACE_ORDER = ['system', 'payroll', 'statutory', 'notifications'];
const ACCOUNT_SECTIONS = ['security', 'sessions'];

export function SettingsPage() {
  const { t } = useTranslation();
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
        label: t(`settings.sections.${c}.label`),
        icon: SECTION_ICONS[c],
        group: 'Workspace' as const,
      })),
      ...ACCOUNT_SECTIONS.map((id) => ({
        id,
        label: t(`settings.sections.${id}.label`),
        icon: SECTION_ICONS[id],
        group: 'Account' as const,
      })),
    ],
    [grouped, t],
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
          <h1 className="page-title">{t('settings.title')}</h1>
          <p className="page-subtitle">{t('settings.subtitle')}</p>
        </div>
        {saved && (
          <span className="flex items-center gap-1 text-sm text-green-600 font-medium">
            <Check className="w-4 h-4" /> {t('common.saved')}
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
              title={t(`settings.sections.${active}.label`)}
              description={t(`settings.sections.${active}.description`)}
              settings={grouped[active] ?? []}
              edits={edits}
              onEdit={(s, v) => setEdits((prev) => ({ ...prev, [`${s.category}/${s.key}`]: v }))}
              onSave={handleSave}
              onDiscard={handleDiscard}
              saving={mutation.isPending}
            />
          ) : (
            <>
              <h2 className="section-title">{t(`settings.sections.${active}.label`)}</h2>
              <p className="text-sm text-gray-500 mt-1 mb-4">{t(`settings.sections.${active}.description`)}</p>
              {active === 'security' ? <SecuritySection /> : <SessionManagement />}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
