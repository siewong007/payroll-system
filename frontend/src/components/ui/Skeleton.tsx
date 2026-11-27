import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';

interface SkeletonProps {
  className?: string;
}

/**
 * Quiet skeleton block that preserves layout geometry while queries load.
 * Respects `prefers-reduced-motion` via `index.css`.
 */
export function Skeleton({ className }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        'rounded-md bg-[var(--surface-subtle)] animate-pulse',
        className,
      )}
    />
  );
}

export function KpiSkeletonGrid({ count = 4, className }: { count?: number; className?: string }) {
  const { t } = useTranslation();
  const colsClass =
    count === 1
      ? 'grid-cols-1'
      : count === 2
        ? 'grid-cols-1 sm:grid-cols-2'
        : count === 3
          ? 'grid-cols-1 sm:grid-cols-2 xl:grid-cols-3'
          : 'grid-cols-1 sm:grid-cols-2 xl:grid-cols-4';
  return (
    <div role="status" aria-busy="true" className={cn('grid gap-3', colsClass, className)}>
      <span className="sr-only">{t('common.loading')}</span>
      {Array.from({ length: count }, (_, idx) => (
        <div key={idx} className="kpi-card space-y-2 min-w-0">
          <Skeleton className="h-3 w-1/2" />
          <Skeleton className="h-6 w-3/4" />
          <Skeleton className="h-2.5 w-2/5" />
        </div>
      ))}
    </div>
  );
}
