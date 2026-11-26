import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Loader2, Upload, X } from 'lucide-react';
import { createClaim, updateClaim, type ClaimWithEmployee } from '@/api/approvals';
import { uploadFile } from '@/api/portal';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { getErrorMessage, todayLocalDate } from '@/lib/utils';
import type { AdminCreateClaimRequest, PaginatedResponse, UpdateClaimRequest } from '@/types';

const ALLOWED_UPLOAD_ACCEPT = '.jpg,.jpeg,.png,.gif,.webp,.pdf,.doc,.docx,.xls,.xlsx';
const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

export function ClaimCrudModal({
  open,
  onClose,
  item,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: ClaimWithEmployee | null;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const formId = useId();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const errorRef = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();
  const [form, setForm] = useState<AdminCreateClaimRequest>({
    employee_id: '',
    title: '',
    description: '',
    amount: 0,
    category: '',
    receipt_url: '',
    receipt_file_name: '',
    expense_date: todayLocalDate(),
  });
  const [amountInput, setAmountInput] = useState('');
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const modalTitle = item
    ? t('approvals.editClaimTitle') + (item.employee_name ? ` - ${item.employee_name}` : '')
    : t('approvals.createClaimTitle');

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        title: item.title,
        description: item.description || '',
        amount: item.amount / 100,
        category: item.category || '',
        receipt_url: item.receipt_url || '',
        receipt_file_name: item.receipt_file_name || '',
        expense_date: item.expense_date,
      });
      setAmountInput((item.amount / 100).toFixed(2));
      setError('');
      return;
    }

    setForm({
      employee_id: '',
      title: '',
      description: '',
      amount: 0,
      category: '',
      receipt_url: '',
      receipt_file_name: '',
      expense_date: todayLocalDate(),
    });
    setAmountInput('');
    setError('');
  }, [item, open]);

  const createMutation = useMutation({
    mutationFn: createClaim,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.createClaimFailed'))),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateClaimRequest }) =>
      updateClaim(id, payload),
    onSuccess: async (updatedClaim) => {
      // The cached value is the paginated envelope, not a bare array — patch
      // inside `.data` or the optimistic update silently does nothing.
      queryClient.setQueriesData<PaginatedResponse<ClaimWithEmployee>>(
        { queryKey: ['approvals-claims'] },
        (current) =>
          current
            ? {
                ...current,
                data: current.data.map((claim) =>
                  claim.id === updatedClaim.id ? { ...claim, ...updatedClaim } : claim,
                ),
              }
            : current,
      );
      await queryClient.refetchQueries({ queryKey: ['approvals-claims'] });
      onSaved();
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.updateClaimFailed'))),
  });

  const handleUpload = async (file: File) => {
    setError('');
    if (file.size > MAX_UPLOAD_BYTES) {
      setError(t('approvals.fileTooLarge'));
      return;
    }
    setUploading(true);
    try {
      const result = await uploadFile(file);
      setForm((prev) => ({
        ...prev,
        receipt_url: result.url,
        receipt_file_name: result.file_name,
      }));
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('approvals.uploadReceiptFailed')));
    } finally {
      setUploading(false);
    }
  };

  const submit = () => {
    const trimmedTitle = form.title.trim();
    const trimmedCategory = form.category?.trim() || '';
    const trimmedDescription = form.description?.trim() || '';
    const parsedAmount = amountInput.trim() ? Number(amountInput) : NaN;
    if (!form.employee_id || !trimmedTitle || !form.expense_date || !Number.isFinite(parsedAmount) || parsedAmount <= 0) {
      setError(t('approvals.claimRequired'));
      return;
    }

    if (item) {
      const receiptUrl = !form.receipt_url && !item.receipt_url ? undefined : form.receipt_url;
      const receiptFileName = !form.receipt_file_name && !item.receipt_file_name ? undefined : form.receipt_file_name;
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          title: trimmedTitle,
          description: trimmedDescription,
          amount: Math.round(parsedAmount * 100),
          category: trimmedCategory,
          receipt_url: receiptUrl,
          receipt_file_name: receiptFileName,
          expense_date: form.expense_date,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      title: trimmedTitle,
      amount: Math.round(parsedAmount * 100),
      description: trimmedDescription || undefined,
      category: trimmedCategory || undefined,
      receipt_url: form.receipt_url || undefined,
      receipt_file_name: form.receipt_file_name || undefined,
    });
  };

  const isSubmitting = createMutation.isPending || updateMutation.isPending;

  useEffect(() => {
    if (error) {
      errorRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [error]);

  return (
    <Modal
      open={open}
      onClose={() => {
        if (!isSubmitting && !uploading) {
          onClose();
        }
      }}
      title={modalTitle}
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={isSubmitting || uploading}
            className="btn-secondary"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={isSubmitting || uploading}
            className="btn-primary"
          >
            {isSubmitting ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                {t('common.saving')}
              </>
            ) : item ? (
              t('common.saveChanges')
            ) : (
              t('approvals.createClaim')
            )}
          </button>
        </div>
      }
    >
      <form
        id={formId}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="space-y-4"
      >
        {error && (
          <div
            ref={errorRef}
            role="alert"
            className="rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300"
          >
            {error}
          </div>
        )}

        <FormField label={t('common.employee')} required>
          {(aria) => (
            <EmployeePicker
              key={item?.id ?? 'new'}
              id={aria.id}
              aria-describedby={aria['aria-describedby']}
              value={form.employee_id}
              onChange={(id) => setForm((prev) => ({ ...prev, employee_id: id }))}
              initialLabel={
                item ? formatEmployeeLabel(item.employee_name, item.employee_number) : undefined
              }
            />
          )}
        </FormField>

        <FormField label={t('common.title')} required>
          {(aria) => (
            <input
              {...aria}
              type="text"
              value={form.title}
              onChange={(event) => setForm((prev) => ({ ...prev, title: event.target.value }))}
              className="form-input"
              required
            />
          )}
        </FormField>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label={t('approvals.amountRm')} required>
            {(aria) => (
              <input
                {...aria}
                type="number"
                inputMode="decimal"
                min="0.01"
                step="0.01"
                value={amountInput}
                onChange={(event) => {
                  const raw = event.target.value;
                  setAmountInput(raw);
                  setForm((prev) => ({ ...prev, amount: raw.trim() ? Number(raw) : 0 }));
                }}
                className="form-input tabular-nums"
                required
              />
            )}
          </FormField>
          <FormField label={t('approvals.expenseDate')} required>
            {(aria) => (
              <input
                {...aria}
                type="date"
                value={form.expense_date}
                onChange={(event) =>
                  setForm((prev) => ({ ...prev, expense_date: event.target.value }))
                }
                className="form-input"
                required
              />
            )}
          </FormField>
        </div>

        <FormField label={t('common.category')}>
          {(aria) => (
            <input
              {...aria}
              type="text"
              value={form.category}
              onChange={(event) => setForm((prev) => ({ ...prev, category: event.target.value }))}
              className="form-input"
            />
          )}
        </FormField>

        <FormField label={t('common.description')}>
          {(aria) => (
            <textarea
              {...aria}
              value={form.description}
              onChange={(event) => setForm((prev) => ({ ...prev, description: event.target.value }))}
              rows={3}
              className="form-input"
            />
          )}
        </FormField>

        <div>
          <span className="form-label">{t('approvals.receipt')}</span>
          <input
            ref={fileInputRef}
            type="file"
            accept={ALLOWED_UPLOAD_ACCEPT}
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.currentTarget.value = '';
              if (file) {
                void handleUpload(file);
              }
            }}
          />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="btn-secondary"
              disabled={uploading}
              aria-busy={uploading}
            >
              {uploading ? (
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
              ) : (
                <Upload className="w-4 h-4" aria-hidden="true" />
              )}{' '}
              {uploading ? t('common.uploading') : t('approvals.uploadReceipt')}
            </button>
            {form.receipt_file_name && (
              <button
                type="button"
                onClick={() =>
                  setForm((prev) => ({ ...prev, receipt_url: '', receipt_file_name: '' }))
                }
                aria-label={t('common.remove')}
                title={t('common.remove')}
                className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
              >
                <X className="w-4 h-4" aria-hidden="true" />
              </button>
            )}
          </div>
          {form.receipt_file_name ? (
            <p className="text-sm text-gray-600 dark:text-slate-300 mt-2 break-all">{form.receipt_file_name}</p>
          ) : (
            <p className="text-xs text-gray-500 dark:text-slate-400 mt-1.5">{t('approvals.uploadHint')}</p>
          )}
        </div>
      </form>
    </Modal>
  );
}
