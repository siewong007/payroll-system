import { AlertCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface UnsavedChangesBarProps {
  isDirty: boolean;
  onDiscard?: () => void;
  onSave?: () => void;
  submitForm?: boolean;
  isSaving?: boolean;
  saveLabel?: string;
}

export function UnsavedChangesBar({
  isDirty,
  onDiscard,
  onSave,
  submitForm = false,
  isSaving = false,
  saveLabel,
}: UnsavedChangesBarProps) {
  const { t } = useTranslation();

  if (!isDirty) return null;

  return (
    <div
      role="status"
      className="animate-slide-up-sm sticky bottom-4 z-30 mt-5 flex flex-col gap-3 rounded-lg border border-amber-300 dark:border-amber-500/35 bg-[var(--card)] px-4 py-2.5 shadow-md sm:flex-row sm:items-center sm:justify-between no-print"
    >
      <div className="flex items-center gap-2 text-[13px] font-medium text-amber-800 dark:text-amber-200">
        <AlertCircle className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />
        <span>{t('ux.unsaved.notice')}</span>
      </div>
      <div className="flex items-center justify-end gap-2">
        {onDiscard && (
          <button
            type="button"
            onClick={onDiscard}
            disabled={isSaving}
            className="btn-secondary !py-1.5 !min-h-8 text-xs"
          >
            {t('common.discard')}
          </button>
        )}
        {(onSave || submitForm) && (
          <button
            type={submitForm ? 'submit' : 'button'}
            onClick={onSave}
            disabled={isSaving}
            className="btn-primary !py-1.5 !min-h-8 text-xs"
          >
            {isSaving ? t('common.saving') : (saveLabel ?? t('common.saveChanges'))}
          </button>
        )}
      </div>
    </div>
  );
}
