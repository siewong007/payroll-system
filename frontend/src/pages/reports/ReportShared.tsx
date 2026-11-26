export function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5 tabular-nums">{value}</p>
    </div>
  );
}

export function SummaryCard({
  label,
  value,
  sub,
  className,
}: {
  label: string;
  value: string;
  sub?: string;
  className?: string;
}) {
  return (
    <div className="card hover-lift !px-4 !py-3 min-w-0">
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide truncate">{label}</p>
      <p
        title={value}
        className={`text-sm font-semibold mt-1 tabular-nums truncate ${className || 'text-gray-900 dark:text-slate-100'}`}
      >
        {value}
      </p>
      {sub && (
        <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5 tabular-nums truncate">{sub}</p>
      )}
    </div>
  );
}
