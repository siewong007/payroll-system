import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';

interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: ReactNode;
  compact?: boolean;
}

export function EmptyState({ icon: Icon, title, description, action, compact = false }: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center justify-center text-center ${compact ? 'py-7 px-4' : 'py-10 px-6'}`}>
      {Icon && (
        <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--muted-foreground)]">
          <Icon className="h-5 w-5" aria-hidden="true" />
        </div>
      )}
      <p className="text-[13px] font-semibold text-[var(--foreground)]">{title}</p>
      {description && (
        <p className="mt-1 max-w-sm text-xs leading-relaxed text-[var(--muted-foreground)]">
          {description}
        </p>
      )}
      {action && <div className="mt-3.5">{action}</div>}
    </div>
  );
}
