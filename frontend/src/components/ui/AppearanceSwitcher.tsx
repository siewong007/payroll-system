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
      className={`inline-flex w-full items-center rounded-xl p-1 text-xs ${
        darkRail
          ? 'bg-white/[0.06] ring-1 ring-white/10'
          : 'bg-gray-100/90 ring-1 ring-gray-200/70 dark:bg-slate-800/80 dark:ring-slate-700'
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
            className={`flex flex-1 items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 font-medium transition-all ${
              active
                ? darkRail
                  ? 'bg-white/15 text-white shadow-sm'
                  : 'bg-white text-gray-900 shadow-sm dark:bg-slate-700 dark:text-white'
                : darkRail
                  ? 'text-slate-400 hover:text-slate-200'
                  : 'text-gray-500 hover:text-gray-800 dark:text-slate-400 dark:hover:text-slate-200'
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
