import { motion } from 'framer-motion';
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
  return (
    <nav
      aria-label="Settings sections"
      className="flex gap-1 overflow-x-auto lg:flex-col lg:gap-0.5 lg:sticky lg:top-6"
    >
      {GROUPS.map((group) => {
        const items = sections.filter((s) => s.group === group);
        if (items.length === 0) return null;
        return (
          <div key={group} className="contents lg:block">
            <p className="hidden lg:block px-3 pt-4 pb-1.5 first:pt-0 text-[10px] font-semibold uppercase tracking-widest text-gray-400">
              {group}
            </p>
            {items.map((item) => {
              const isActive = item.id === active;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => onSelect(item.id)}
                  aria-current={isActive ? 'true' : undefined}
                  className={`relative flex items-center gap-2.5 px-3 py-2 rounded-xl text-sm whitespace-nowrap transition-all-fast ${
                    isActive
                      ? 'text-gray-900 font-medium'
                      : 'text-gray-500 hover:text-gray-800 hover:bg-black/[0.03]'
                  }`}
                >
                  {isActive && (
                    <motion.span
                      layoutId="settings-nav-active"
                      className="absolute inset-0 rounded-xl bg-[var(--accent-soft)] ring-1 ring-black/5"
                      transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                    />
                  )}
                  <item.icon
                    className={`relative w-4 h-4 ${isActive ? 'text-[var(--accent-1)]' : 'text-gray-400'}`}
                  />
                  <span className="relative">{item.label}</span>
                </button>
              );
            })}
          </div>
        );
      })}
    </nav>
  );
}
