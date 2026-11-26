import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Loader2, Upload, X } from 'lucide-react';
import {
  createLeaveRequest,
  updateLeaveRequest,
  type LeaveRequestWithEmployee,
} from '@/api/approvals';
import { getPortalWorkingDays, uploadFile } from '@/api/portal';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { getErrorMessage } from '@/lib/utils';
import type { AdminCreateLeaveRequest, LeaveType, UpdateLeaveRequest } from '@/types';

const ALLOWED_UPLOAD_ACCEPT = '.jpg,.jpeg,.png,.gif,.webp,.pdf,.doc,.docx,.xls,.xlsx';
const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

function inclusiveDaysBetween(start: string, end: string): number | null {
  if (!start || !end || end < start) return null;
  const [sy, sm, sd] = start.split('-').map(Number);
  const [ey, em, ed] = end.split('-').map(Number);
  if (!sy || !sm || !sd || !ey || !em || !ed) return null;
  const diff = Math.round((Date.UTC(ey, em - 1, ed) - Date.UTC(sy, sm - 1, sd)) / 86_400_000) + 1;
  return diff >= 1 ? diff : null;
}

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
  const formId = useId();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const errorRef = useRef<HTMLDivElement>(null);
  const latestLeaveTypesRef = useRef<LeaveType[]>(leaveTypes);
  useEffect(() => {
    latestLeaveTypesRef.current = leaveTypes;
  }, [leaveTypes]);
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
  const [daysInput, setDaysInput] = useState('1');
  const [datesDirty, setDatesDirty] = useState(false);
  const [daysManuallyEdited, setDaysManuallyEdited] = useState(false);
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const modalTitle = item
    ? t('approvals.editLeaveTitle') + (item.employee_name ? ` - ${item.employee_name}` : '')
    : t('approvals.createLeaveTitle');

  useEffect(() => {
    if (!open) {
      return;
    }

    setDatesDirty(false);
    setDaysManuallyEdited(false);
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
      setDaysInput(String(Number(item.days)));
      setError('');
      return;
    }

    // No default employee: whoever sorts first is exactly the wrong person to
    // file a request against, and the submit guard already demands a choice.
    setForm({
      employee_id: '',
      leave_type_id: latestLeaveTypesRef.current[0]?.id || '',
      start_date: '',
      end_date: '',
      days: 1,
      reason: '',
      attachment_url: '',
      attachment_name: '',
    });
    setDaysInput('1');
    setError('');
  }, [item, open]);

  useEffect(() => {
    if (!item && open && !form.leave_type_id && leaveTypes[0]?.id) {
      setForm((prev) => (prev.leave_type_id ? prev : { ...prev, leave_type_id: leaveTypes[0].id }));
    }
  }, [form.leave_type_id, item, leaveTypes, open]);

  const validDateRange = Boolean(
    form.start_date && form.end_date && form.end_date >= form.start_date,
  );
  const { data: serverWorkingDays } = useQuery({
    queryKey: ['portal-working-days', form.start_date, form.end_date],
    queryFn: () => getPortalWorkingDays(form.start_date, form.end_date),
    enabled: open && validDateRange,
  });

  useEffect(() => {
    if (!open || !validDateRange || !serverWorkingDays || daysManuallyEdited) return;
    if (item && !datesDirty) return;
    const workingDays = serverWorkingDays.working_days;
    setForm((prev) => ({ ...prev, days: workingDays }));
    setDaysInput(String(workingDays));
  }, [open, validDateRange, serverWorkingDays, item, datesDirty, daysManuallyEdited]);

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

  const handleDateChange = (field: 'start_date' | 'end_date', value: string) => {
    setDatesDirty(true);
    setDaysManuallyEdited(false);
    setForm((prev) => {
      const next = { ...prev, [field]: value };
      const computed = inclusiveDaysBetween(next.start_date, next.end_date);
      if (computed !== null) {
        next.days = computed;
        setDaysInput(String(computed));
      }
      return next;
    });
  };

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
    const parsedDays = daysInput.trim() ? Number(daysInput) : NaN;
    if (
      !form.employee_id ||
      !form.leave_type_id ||
      !form.start_date ||
      !form.end_date ||
      !Number.isFinite(parsedDays) ||
      parsedDays <= 0
    ) {
      setError(t('approvals.leaveRequired'));
      return;
    }

    if (form.end_date < form.start_date) {
      setError(t('approvals.endBeforeStart'));
      return;
    }

    const trimmedReason = form.reason?.trim() || '';

    if (item) {
      const attachmentUrl = !form.attachment_url && !item.attachment_url ? undefined : form.attachment_url;
      const attachmentName = !form.attachment_name && !item.attachment_name ? undefined : form.attachment_name;
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          leave_type_id: form.leave_type_id,
          start_date: form.start_date,
          end_date: form.end_date,
          days: parsedDays,
          reason: trimmedReason,
          attachment_url: attachmentUrl,
          attachment_name: attachmentName,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      days: parsedDays,
      reason: trimmedReason || undefined,
      attachment_url: form.attachment_url || undefined,
      attachment_name: form.attachment_name || undefined,
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
              t('approvals.createRequest')
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
            /* Keyed on the record so reopening the modal for a different request
               inside the exit animation cannot leave the previous name on screen. */
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

        <FormField label={t('approvals.leaveType')} required>
          {(aria) => (
            <select
              {...aria}
              value={form.leave_type_id}
              onChange={(event) =>
                setForm((prev) => ({ ...prev, leave_type_id: event.target.value }))
              }
              className="form-input"
              required
            >
              <option value="">{t('approvals.selectLeaveType')}</option>
              {leaveTypes.map((leaveType) => (
                <option key={leaveType.id} value={leaveType.id}>
                  {leaveType.name}
                </option>
              ))}
            </select>
          )}
        </FormField>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label={t('common.startDate')} required>
            {(aria) => (
              <input
                {...aria}
                type="date"
                value={form.start_date}
                onChange={(event) => handleDateChange('start_date', event.target.value)}
                className="form-input"
                required
              />
            )}
          </FormField>
          <FormField label={t('common.endDate')} required>
            {(aria) => (
              <input
                {...aria}
                type="date"
                value={form.end_date}
                min={form.start_date || undefined}
                onChange={(event) => handleDateChange('end_date', event.target.value)}
                className="form-input"
                required
              />
            )}
          </FormField>
        </div>

        <FormField label={t('approvals.days')} required>
          {(aria) => (
            <input
              {...aria}
              type="number"
              inputMode="decimal"
              min="0.5"
              step="0.5"
              value={daysInput}
              onChange={(event) => {
                const raw = event.target.value;
                setDaysManuallyEdited(true);
                setDaysInput(raw);
                setForm((prev) => ({ ...prev, days: raw.trim() ? Number(raw) : 0 }));
              }}
              className="form-input tabular-nums"
              required
            />
          )}
        </FormField>

        <FormField label={t('common.reason')}>
          {(aria) => (
            <textarea
              {...aria}
              value={form.reason}
              onChange={(event) => setForm((prev) => ({ ...prev, reason: event.target.value }))}
              rows={3}
              className="form-input"
            />
          )}
        </FormField>

        <div>
          <span className="form-label">{t('approvals.attachment')}</span>
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
              {uploading ? t('common.uploading') : t('approvals.uploadFile')}
            </button>
            {form.attachment_name && (
              <button
                type="button"
                onClick={() =>
                  setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' }))
                }
                aria-label={t('common.remove')}
                title={t('common.remove')}
                className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
              >
                <X className="w-4 h-4" aria-hidden="true" />
              </button>
            )}
          </div>
          {form.attachment_name ? (
            <p className="text-sm text-gray-600 dark:text-slate-300 mt-2 break-all">{form.attachment_name}</p>
          ) : (
            <p className="text-xs text-gray-500 dark:text-slate-400 mt-1.5">{t('approvals.uploadHint')}</p>
          )}
        </div>
      </form>
    </Modal>
  );
}
