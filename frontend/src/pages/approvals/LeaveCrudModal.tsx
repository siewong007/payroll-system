import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Upload, X } from 'lucide-react';
import {
  createLeaveRequest,
  updateLeaveRequest,
  type LeaveRequestWithEmployee,
} from '@/api/approvals';
import { uploadFile } from '@/api/portal';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { Modal } from '@/components/ui/Modal';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { getErrorMessage } from '@/lib/utils';
import type { AdminCreateLeaveRequest, LeaveType, UpdateLeaveRequest } from '@/types';

export function LeaveCrudModal({
  open,
  onClose,
  item,
  leaveTypes,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: LeaveRequestWithEmployee | null;
  leaveTypes: LeaveType[];
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState<AdminCreateLeaveRequest>({
    employee_id: '',
    leave_type_id: '',
    start_date: '',
    end_date: '',
    days: 1,
    reason: '',
    attachment_url: '',
    attachment_name: '',
  });
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const modalTitle = item
    ? t('approvals.editLeaveTitle') + (item.employee_name ? ` - ${item.employee_name}` : '')
    : t('approvals.createLeaveTitle');

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        leave_type_id: item.leave_type_id,
        start_date: item.start_date,
        end_date: item.end_date,
        days: Number(item.days),
        reason: item.reason || '',
        attachment_url: item.attachment_url || '',
        attachment_name: item.attachment_name || '',
      });
      setError('');
      return;
    }

    // No default employee: whoever sorts first is exactly the wrong person to
    // file a request against, and the submit guard already demands a choice.
    setForm({
      employee_id: '',
      leave_type_id: leaveTypes[0]?.id || '',
      start_date: '',
      end_date: '',
      days: 1,
      reason: '',
      attachment_url: '',
      attachment_name: '',
    });
    setError('');
  }, [item, leaveTypes, open]);

  const createMutation = useMutation({
    mutationFn: createLeaveRequest,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.createLeaveFailed'))),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateLeaveRequest }) =>
      updateLeaveRequest(id, payload),
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.updateLeaveFailed'))),
  });

  const handleUpload = async (file: File) => {
    setError('');
    setUploading(true);
    try {
      const result = await uploadFile(file);
      setForm((prev) => ({
        ...prev,
        attachment_url: result.url,
        attachment_name: result.file_name,
      }));
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('approvals.uploadAttachmentFailed')));
    } finally {
      setUploading(false);
    }
  };

  const submit = () => {
    if (
      !form.employee_id ||
      !form.leave_type_id ||
      !form.start_date ||
      !form.end_date ||
      form.days <= 0
    ) {
      setError(t('approvals.leaveRequired'));
      return;
    }

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          leave_type_id: form.leave_type_id,
          start_date: form.start_date,
          end_date: form.end_date,
          days: form.days,
          reason: form.reason,
          attachment_url: form.attachment_url,
          attachment_name: form.attachment_name,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      reason: form.reason || undefined,
      attachment_url: form.attachment_url || undefined,
      attachment_name: form.attachment_name || undefined,
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
                : t('approvals.createRequest')}
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
          {/* Keyed on the record so reopening the modal for a different request
              inside the exit animation cannot leave the previous name on screen. */}
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
          <label className="form-label">{t('approvals.leaveType')} *</label>
          <select
            value={form.leave_type_id}
            onChange={(event) =>
              setForm((prev) => ({ ...prev, leave_type_id: event.target.value }))
            }
            className="form-input"
          >
            <option value="">{t('approvals.selectLeaveType')}</option>
            {leaveTypes.map((leaveType) => (
              <option key={leaveType.id} value={leaveType.id}>
                {leaveType.name}
              </option>
            ))}
          </select>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="form-label">{t('common.startDate')} *</label>
            <input
              type="date"
              value={form.start_date}
              onChange={(event) =>
                setForm((prev) => ({ ...prev, start_date: event.target.value }))
              }
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">{t('common.endDate')} *</label>
            <input
              type="date"
              value={form.end_date}
              onChange={(event) => setForm((prev) => ({ ...prev, end_date: event.target.value }))}
              className="form-input"
            />
          </div>
        </div>

        <div>
          <label className="form-label">{t('approvals.days')} *</label>
          <input
            type="number"
            min="0.5"
            step="0.5"
            value={form.days}
            onChange={(event) =>
              setForm((prev) => ({ ...prev, days: Number(event.target.value) }))
            }
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">{t('common.reason')}</label>
          <textarea
            value={form.reason}
            onChange={(event) => setForm((prev) => ({ ...prev, reason: event.target.value }))}
            rows={3}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">{t('approvals.attachment')}</label>
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
              {uploading ? t('common.uploading') : t('approvals.uploadFile')}
            </button>
            {form.attachment_name && (
              <button
                type="button"
                onClick={() =>
                  setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' }))
                }
                className="p-2 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          {form.attachment_name && (
            <p className="text-sm text-gray-500 mt-2">{form.attachment_name}</p>
          )}
        </div>
      </div>
    </Modal>
  );
}
