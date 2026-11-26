import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';

interface SkeletonProps {
  className?: string;
}

/**
 * Shimmer placeholder block that preserves layout geometry while queries load.
 * Respects `prefers-reduced-motion` via `.shimmer` in `index.css`.
 */
export function Skeleton({ className }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        'relative overflow-hidden rounded-lg bg-gray-200/75 dark:bg-slate-800/80 shimmer',
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
    <div role="status" aria-busy="true" className={cn('grid gap-4', colsClass, className)}>
      <span className="sr-only">{t('common.loading')}</span>
      {Array.from({ length: count }, (_, idx) => (
        <div key={idx} className="card space-y-2.5 min-w-0">
          <Skeleton className="h-3.5 w-1/2" />
          <Skeleton className="h-7 w-3/4" />
          <Skeleton className="h-3 w-2/5" />
        </div>
      ))}
    </div>
  );
}
