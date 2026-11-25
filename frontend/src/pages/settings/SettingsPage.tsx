import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchParams } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  Bell,
  Calculator,
  Check,
  Landmark,
  Mail,
  MonitorSmartphone,
  ShieldCheck,
  SlidersHorizontal,
  Smartphone,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { getSettings, bulkUpdateSettings } from '@/api/settings';
import type { CompanySetting, SettingUpdate } from '@/types';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { SessionManagement } from '@/components/SessionManagement';
import { AppInstallCard } from '@/components/pwa/AppInstallCard';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import { getErrorMessage } from '@/lib/utils';
import { SettingsNav, type SettingsNavItem } from './SettingsNav';
import { CompanyCategorySection } from './CompanyCategorySection';
import { SecuritySection } from './SecuritySection';

const SECTION_ICONS: Record<string, LucideIcon> = {
  system: SlidersHorizontal,
  payroll: Calculator,
  statutory: Landmark,
  notifications: Bell,
  email: Mail,
  security: ShieldCheck,
  sessions: MonitorSmartphone,
  app: Smartphone,
};

const WORKSPACE_ORDER = ['system', 'payroll', 'statutory', 'notifications', 'email'];
const ACCOUNT_SECTIONS = ['security', 'sessions', 'app'];

export function SettingsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const confirm = useConfirm();
  const [searchParams, setSearchParams] = useSearchParams();
  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saved, setSaved] = useState(false);

  useUnsavedChanges(Object.keys(edits).length > 0);

  const {
    data: settings,
    isLoading,
    isError,
    error: loadError,
    refetch,
  } = useQuery({
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

  const workspaceCategories = useMemo(
    () => [
      ...WORKSPACE_ORDER.filter((c) => grouped[c]?.length),
      ...Object.keys(grouped).filter((c) => !WORKSPACE_ORDER.includes(c) && grouped[c]?.length),
    ],
    [grouped],
  );

  const sections = useMemo<SettingsNavItem[]>(
    () => [
      ...workspaceCategories.map((c) => ({
        id: c,
        label: t(`settings.sections.${c}.label`, { defaultValue: c }),
        icon: SECTION_ICONS[c] ?? SlidersHorizontal,
        group: 'Workspace' as const,
      })),
      ...ACCOUNT_SECTIONS.map((id) => ({
        id,
        label: t(`settings.sections.${id}.label`),
        icon: SECTION_ICONS[id],
        group: 'Account' as const,
      })),
    ],
    [workspaceCategories, t],
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
      toast.success(t('common.saved'));
      setTimeout(() => setSaved(false), 2000);
    },
    onError: (err: unknown) => {
      toast.error(getErrorMessage(err, t('settings.saveFailed')));
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

  const handleSelectSection = async (id: string) => {
    if (id === active) return;
    if (activeEdits().length > 0) {
      const ok = await confirm({
        title: t('common.discard'),
        description: t('settings.discardConfirm'),
        confirmLabel: t('common.discard'),
        variant: 'warning',
      });
      if (!ok) return;
      handleDiscard();
    }
    setSearchParams({ section: id });
  };

  const isWorkspace = workspaceCategories.includes(active);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="spinner" />
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title={t('settings.title')}
        subtitle={t('settings.subtitle')}
        actions={
          saved ? (
            <span className="flex items-center gap-1 text-sm text-green-600 dark:text-emerald-400 font-medium">
              <Check className="w-4 h-4" /> {t('common.saved')}
            </span>
          ) : undefined
        }
      />

      {isError && (
        <div className="card mb-6">
          <EmptyState
            compact
            icon={AlertTriangle}
            title={t('settings.loadFailed')}
            description={getErrorMessage(loadError, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      )}

      <div className="lg:grid lg:grid-cols-[220px_1fr] lg:gap-6 lg:items-start">
        <SettingsNav
          sections={sections}
          active={active}
          onSelect={(id) => { void handleSelectSection(id); }}
        />

        <div key={active} className="mt-4 lg:mt-0 animate-fade-up">
          {isWorkspace ? (
            <CompanyCategorySection
              title={t(`settings.sections.${active}.label`, { defaultValue: active })}
              description={t(`settings.sections.${active}.description`, { defaultValue: '' })}
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
              <p className="text-sm text-gray-500 dark:text-slate-400 mt-1 mb-4">{t(`settings.sections.${active}.description`)}</p>
              {active === 'security' ? (
                <SecuritySection />
              ) : active === 'sessions' ? (
                <SessionManagement />
              ) : (
                <div className="space-y-6">
                  <AppInstallCard />
                  <NotificationActivationCard />
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
