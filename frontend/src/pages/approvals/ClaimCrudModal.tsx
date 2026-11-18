import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Upload, X } from 'lucide-react';
import { createClaim, updateClaim, type ClaimWithEmployee } from '@/api/approvals';
import { uploadFile } from '@/api/portal';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { Modal } from '@/components/ui/Modal';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { getErrorMessage, todayLocalDate } from '@/lib/utils';
import type { AdminCreateClaimRequest, PaginatedResponse, UpdateClaimRequest } from '@/types';

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
  const fileInputRef = useRef<HTMLInputElement>(null);
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
    if (!form.employee_id || !form.title || !form.expense_date || form.amount <= 0) {
      setError(t('approvals.claimRequired'));
      return;
    }

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          title: form.title,
          description: form.description,
          amount: Math.round(form.amount * 100),
          category: form.category,
          receipt_url: form.receipt_url,
          receipt_file_name: form.receipt_file_name,
          expense_date: form.expense_date,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      amount: Math.round(form.amount * 100),
      description: form.description || undefined,
      category: form.category || undefined,
      receipt_url: form.receipt_url || undefined,
      receipt_file_name: form.receipt_file_name || undefined,
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={modalTitle}
      footer={
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn-secondary">
            {t('common.cancel')}
          </button>
          <button
            onClick={submit}
            disabled={createMutation.isPending || updateMutation.isPending || uploading}
            className="btn-primary"
          >
            {createMutation.isPending || updateMutation.isPending
              ? t('common.saving')
              : item
                ? t('common.saveChanges')
                : t('approvals.createClaim')}
          </button>
        </div>
      }
    >
      <div className="space-y-4">
        {error && (
          <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
            {error}
          </div>
        )}

        <div>
          <label className="form-label">{t('common.employee')} *</label>
          <EmployeePicker
            key={item?.id ?? 'new'}
            value={form.employee_id}
            onChange={(id) => setForm((prev) => ({ ...prev, employee_id: id }))}
            initialLabel={
              item ? formatEmployeeLabel(item.employee_name, item.employee_number) : undefined
            }
          />
        </div>

        <div>
          <label className="form-label">{t('common.title')} *</label>
          <input
            type="text"
            value={form.title}
            onChange={(event) => setForm((prev) => ({ ...prev, title: event.target.value }))}
            className="form-input"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="form-label">{t('approvals.amountRm')} *</label>
            <input
              type="number"
              min="0.01"
              step="0.01"
              value={form.amount}
              onChange={(event) =>
                setForm((prev) => ({ ...prev, amount: Number(event.target.value) }))
              }
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">{t('approvals.expenseDate')} *</label>
            <input
              type="date"
              value={form.expense_date}
              onChange={(event) =>
                setForm((prev) => ({ ...prev, expense_date: event.target.value }))
              }
              className="form-input"
            />
          </div>
        </div>

        <div>
          <label className="form-label">{t('common.category')}</label>
          <input
            type="text"
            value={form.category}
            onChange={(event) => setForm((prev) => ({ ...prev, category: event.target.value }))}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">{t('common.description')}</label>
          <textarea
            value={form.description}
            onChange={(event) => setForm((prev) => ({ ...prev, description: event.target.value }))}
            rows={3}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">{t('approvals.receipt')}</label>
          <input
            ref={fileInputRef}
            type="file"
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
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
            >
              <Upload className="w-4 h-4" />{' '}
              {uploading ? t('common.uploading') : t('approvals.uploadReceipt')}
            </button>
            {form.receipt_file_name && (
              <button
                type="button"
                onClick={() =>
                  setForm((prev) => ({ ...prev, receipt_url: '', receipt_file_name: '' }))
                }
                className="p-2 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          {form.receipt_file_name && (
            <p className="text-sm text-gray-500 mt-2">{form.receipt_file_name}</p>
          )}
        </div>
      </div>
    </Modal>
  );
}
