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
      className="animate-slide-up-sm sticky bottom-4 z-30 mt-6 flex flex-col gap-3 rounded-2xl border border-amber-200/80 dark:border-amber-500/30 bg-white/95 dark:bg-slate-900/95 px-4 py-3 shadow-xl backdrop-blur-xl sm:flex-row sm:items-center sm:justify-between no-print"
    >
      <div className="flex items-center gap-2.5 text-sm font-medium text-amber-800 dark:text-amber-200">
        <AlertCircle className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />
        <span>{t('ux.unsaved.notice')}</span>
      </div>
      <div className="flex items-center justify-end gap-2">
        {onDiscard && (
          <button
            type="button"
            onClick={onDiscard}
            disabled={isSaving}
            className="btn-secondary !py-2 !min-h-9 text-xs"
          >
            {t('common.discard')}
          </button>
        )}
        {(onSave || submitForm) && (
          <button
            type={submitForm ? 'submit' : 'button'}
            onClick={onSave}
            disabled={isSaving}
            className="btn-primary !py-2 !min-h-9 text-xs"
          >
            {isSaving ? t('common.saving') : (saveLabel ?? t('common.saveChanges'))}
          </button>
        )}
      </div>
    </div>
  );
}
