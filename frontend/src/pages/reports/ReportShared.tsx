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
  className,
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 px-4 py-3">
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      <p className={`text-sm font-semibold mt-1 tabular-nums ${className || 'text-gray-900 dark:text-slate-100'}`}>
        {value}
      </p>
    </div>
  );
}
