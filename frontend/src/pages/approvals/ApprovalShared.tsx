import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Trash2, X } from 'lucide-react';

export function RequestStatusBadge({ status }: { status: string }) {
  const { t } = useTranslation();
  const cls: Record<string, string> = {
    pending: 'badge-pending',
    approved: 'badge-approved',
    rejected: 'badge-rejected',
    cancelled: 'badge-cancelled',
    draft: 'badge-draft',
    processed: 'badge-processed',
  };
  return (
    <span className={`badge ${cls[status] || 'badge-draft'}`}>
      {t(`enums.requestStatus.${status}`, { defaultValue: status })}
    </span>
  );
}

export function SummaryField({
  label,
  value,
  children,
}: {
  label: string;
  value?: string;
  children?: ReactNode;
}) {
  return (
    <div className="min-w-0">
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      {children ?? (
        <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5 break-words">
          {value || '\u2014'}
        </p>
      )}
    </div>
  );
}

export function ActionButtons({
  onEdit,
  onCancel,
  onDelete,
}: {
  onEdit?: () => void;
  onCancel?: () => void;
  onDelete?: () => void;
}) {
  const { t } = useTranslation();
  if (!onEdit && !onCancel && !onDelete) {
    return <span className="text-gray-300 dark:text-slate-600">{'\u2014'}</span>;
  }

  return (
    <div className="flex items-center justify-center gap-1">
      {onEdit && (
        <button
          type="button"
          onClick={onEdit}
          className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-black dark:hover:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors"
          aria-label={t('common.edit')}
          title={t('common.edit')}
        >
          <Pencil className="w-4 h-4" />
        </button>
      )}
      {onDelete && (
        <button
          type="button"
          onClick={onDelete}
          className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-red-500 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-500/15 transition-colors"
          aria-label={t('common.delete')}
          title={t('common.delete')}
        >
          <Trash2 className="w-4 h-4" />
        </button>
      )}
      {onCancel && (
        <button
          type="button"
          onClick={onCancel}
          className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-400 dark:text-slate-500 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 transition-colors"
          aria-label={t('common.cancel')}
          title={t('common.cancel')}
        >
          <X className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
