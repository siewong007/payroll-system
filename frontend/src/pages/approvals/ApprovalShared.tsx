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
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      {children ?? <p className="text-sm font-medium text-gray-900 mt-0.5">{value || '\u2014'}</p>}
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
    return <span className="text-gray-300">—</span>;
  }

  return (
    <div className="flex items-center justify-center gap-2">
      {onEdit && (
        <button
          type="button"
          onClick={onEdit}
          className="p-1.5 rounded-lg text-gray-500 hover:text-black hover:bg-gray-100 transition-colors"
          title={t('common.edit')}
        >
          <Pencil className="w-4 h-4" />
        </button>
      )}
      {onDelete && (
        <button
          type="button"
          onClick={onDelete}
          className="p-1.5 rounded-lg text-red-500 hover:text-red-700 hover:bg-red-50 transition-colors"
          title={t('common.delete')}
        >
          <Trash2 className="w-4 h-4" />
        </button>
      )}
      {onCancel && (
        <button
          type="button"
          onClick={onCancel}
          className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 transition-colors"
          title={t('common.cancel')}
        >
          <X className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
