import { Fragment, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { LucideIcon } from 'lucide-react';

export interface SettingsNavItem {
  id: string;
  label: string;
  icon: LucideIcon;
  group: 'Workspace' | 'Account';
}

const GROUPS = ['Workspace', 'Account'] as const;

export function SettingsNav({
  sections,
  active,
  onSelect,
}: {
  sections: SettingsNavItem[];
  active: string;
  onSelect: (id: string) => void;
}) {
  const { t } = useTranslation();
  const activeRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    const el = activeRef.current;
    if (el && typeof el.scrollIntoView === 'function') {
      el.scrollIntoView({ block: 'nearest', inline: 'center', behavior: 'smooth' });
    }
  }, [active]);

  const populatedGroups = GROUPS.map((group) => ({
    group,
    items: sections.filter((s) => s.group === group),
  })).filter((g) => g.items.length > 0);

  return (
    <nav
      aria-label={t('settings.navLabel')}
      className="flex gap-1.5 overflow-x-auto pb-2 lg:pb-0 lg:flex-col lg:gap-0.5 lg:sticky lg:top-6 scroll-px-4"
    >
      {populatedGroups.map(({ group, items }, idx) => (
        <Fragment key={group}>
          {idx > 0 && (
            <div
              aria-hidden="true"
              className="lg:hidden w-px h-6 self-center bg-gray-200 dark:bg-slate-700 shrink-0 mx-1"
            />
          )}
          <div className="contents lg:block lg:pt-3 first:lg:pt-0">
            <p className="hidden lg:block px-3 pb-1.5 text-[11px] font-semibold uppercase tracking-widest text-gray-600 dark:text-slate-400">
              {t(`settings.groups.${group.toLowerCase()}`)}
            </p>
            {items.map((item) => {
              const isActive = item.id === active;
              return (
                <button
                  key={item.id}
                  ref={isActive ? activeRef : undefined}
                  type="button"
                  onClick={() => onSelect(item.id)}
                  aria-current={isActive ? 'page' : undefined}
                  className={`relative flex items-center gap-2.5 min-h-11 lg:min-h-10 lg:w-full shrink-0 px-3 py-2 rounded-xl text-sm whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-all-fast ${
                    isActive
                      ? 'text-gray-900 dark:text-slate-100 font-medium'
                      : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]'
                  }`}
                >
                  {isActive && (
                    <span className="absolute inset-0 rounded-xl bg-[var(--accent-soft)] ring-1 ring-black/5 dark:ring-white/10" />
                  )}
                  <item.icon
                    aria-hidden="true"
                    className={`relative w-4 h-4 shrink-0 ${isActive ? 'text-[var(--accent-1)]' : 'text-gray-500 dark:text-slate-400'}`}
                  />
                  <span className="relative">{item.label}</span>
                </button>
              );
            })}
          </div>
        </Fragment>
      ))}
    </nav>
  );
}
