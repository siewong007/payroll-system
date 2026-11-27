import { Monitor, Moon, Sun } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAppearance, type AppearanceMode } from '@/context/ThemeContext';

const OPTIONS: { mode: AppearanceMode; icon: typeof Sun; labelKey: string }[] = [
  { mode: 'light', icon: Sun, labelKey: 'ux.appearance.light' },
  { mode: 'dark', icon: Moon, labelKey: 'ux.appearance.dark' },
  { mode: 'system', icon: Monitor, labelKey: 'ux.appearance.system' },
];

export function AppearanceSwitcher({ darkRail = false }: { darkRail?: boolean }) {
  const { t } = useTranslation();
  const { mode, setMode } = useAppearance();

  return (
    <div
      role="group"
      aria-label={t('ux.appearance.label')}
      className={`inline-flex w-full items-center rounded-md p-0.5 text-xs ${
        darkRail
          ? 'bg-white/[0.06] border border-white/10'
          : 'bg-[var(--surface-subtle)] border border-[var(--border)]'
      }`}
    >
      {OPTIONS.map(({ mode: itemMode, icon: Icon, labelKey }) => {
        const active = mode === itemMode;
        return (
          <button
            key={itemMode}
            type="button"
            onClick={() => setMode(itemMode)}
            aria-pressed={active}
            className={`flex flex-1 items-center justify-center gap-1.5 rounded-[5px] px-2 py-1 font-medium transition-colors ${
              active
                ? darkRail
                  ? 'bg-white/15 text-white'
                  : 'bg-[var(--card)] text-[var(--foreground)] shadow-[0_1px_2px_rgba(0,0,0,0.05)]'
                : darkRail
                  ? 'text-zinc-400 hover:text-zinc-200'
                  : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
            }`}
          >
            <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
            <span className="truncate">{t(labelKey)}</span>
          </button>
        );
      })}
    </div>
  );
}
