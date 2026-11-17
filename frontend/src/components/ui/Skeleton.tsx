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

export function KpiSkeletonGrid({ count = 4 }: { count?: number }) {
  return (
    <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
      {Array.from({ length: count }, (_, idx) => (
        <div key={idx} className="card space-y-2.5">
          <Skeleton className="h-3.5 w-24" />
          <Skeleton className="h-7 w-32" />
          <Skeleton className="h-3 w-20" />
        </div>
      ))}
    </div>
  );
}
