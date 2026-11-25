import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createOvertimeRequest,
  updateOvertimeRequest,
  type OvertimeWithEmployee,
} from '@/api/approvals';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import {
  OT_DEFAULT_END,
  OT_DEFAULT_HOURS,
  OT_DEFAULT_START,
  calculateOvertimeHours,
} from '@/lib/overtime';
import { getErrorMessage } from '@/lib/utils';
import type {
  AdminCreateOvertimeRequest,
  CreateOvertimeRequest,
  PaginatedResponse,
  UpdateOvertimeRequest,
} from '@/types';

export function OvertimeCrudModal({
  open,
  onClose,
  item,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: OvertimeWithEmployee | null;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const formId = useId();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<AdminCreateOvertimeRequest>({
    employee_id: '',
    ot_date: '',
    start_time: OT_DEFAULT_START,
    end_time: OT_DEFAULT_END,
    hours: OT_DEFAULT_HOURS,
    ot_type: 'normal',
    reason: '',
  });
  const [error, setError] = useState('');
  const modalTitle = item
    ? t('approvals.editOvertimeTitle') + (item.employee_name ? ` - ${item.employee_name}` : '')
    : t('approvals.createOvertimeTitle');

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        ot_date: item.ot_date,
        start_time: item.start_time?.slice(0, 5),
        end_time: item.end_time?.slice(0, 5),
        hours: Number(item.hours),
        ot_type: item.ot_type as CreateOvertimeRequest['ot_type'],
        reason: item.reason || '',
      });
      setError('');
      return;
    }

    // The times are seeded with what the fields display. Holding '' behind a
    // rendered 18:00 is what made Submit refuse a form that looked complete.
    // The OT *date* stays empty on purpose: one deliberate input is required.
    setForm({
      employee_id: '',
      ot_date: '',
      start_time: OT_DEFAULT_START,
      end_time: OT_DEFAULT_END,
      hours: OT_DEFAULT_HOURS,
      ot_type: 'normal',
      reason: '',
    });
    setError('');
  }, [item, open]);

  const createMutation = useMutation({
    mutationFn: createOvertimeRequest,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.createOvertimeFailed'))),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateOvertimeRequest }) =>
      updateOvertimeRequest(id, payload),
    onSuccess: async (updatedOvertime) => {
      // See the claims equivalent: the cache holds the paginated envelope.
      queryClient.setQueriesData<PaginatedResponse<OvertimeWithEmployee>>(
        { queryKey: ['approvals-overtime'] },
        (current) =>
          current
            ? {
                ...current,
                data: current.data.map((overtime) =>
                  overtime.id === updatedOvertime.id
                    ? { ...overtime, ...updatedOvertime }
                    : overtime,
                ),
              }
            : current,
      );
      await queryClient.refetchQueries({ queryKey: ['approvals-overtime'] });
      onSaved();
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('approvals.updateOvertimeFailed'))),
  });

  const updateTime = (field: 'start_time' | 'end_time', value: string) => {
    setForm((prev) => {
      const next = { ...prev, [field]: value };
      next.hours = calculateOvertimeHours(
        field === 'start_time' ? value : prev.start_time,
        field === 'end_time' ? value : prev.end_time,
      );
      return next;
    });
  };

  const submit = () => {
    if (!form.employee_id || !form.ot_date) {
      setError(t('approvals.overtimeRequired'));
      return;
    }
    if (!form.start_time || !form.end_time || form.hours <= 0) {
      setError(t('approvals.timesMustDiffer'));
      return;
    }

    const trimmedReason = form.reason?.trim() || '';

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          ot_date: form.ot_date,
          start_time: form.start_time,
          end_time: form.end_time,
          hours: form.hours,
          ot_type: form.ot_type,
          reason: trimmedReason,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      reason: trimmedReason || undefined,
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={modalTitle}
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={createMutation.isPending || updateMutation.isPending}
            className="btn-primary"
          >
            {createMutation.isPending || updateMutation.isPending
              ? t('common.saving')
              : item
                ? t('common.saveChanges')
                : t('approvals.createOvertime')}
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
          <div role="alert" className="rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300">
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

        <FormField label={t('approvals.otDate')} required>
          {(aria) => (
            <input
              {...aria}
              type="date"
              value={form.ot_date}
              onChange={(event) => setForm((prev) => ({ ...prev, ot_date: event.target.value }))}
              className="form-input"
              required
            />
          )}
        </FormField>

        <div>
          <label className="form-label">{t('approvals.otType')} *</label>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
            {(['normal', 'rest_day', 'public_holiday'] as const).map((type) => (
              <label
                key={type}
                className={`flex flex-col items-center gap-1 px-3 py-3 rounded-lg border-2 cursor-pointer transition-all text-center focus-within:ring-2 focus-within:ring-black dark:focus-within:ring-slate-400 ${
                  form.ot_type === type
                    ? type === 'public_holiday'
                      ? 'border-red-400 bg-red-50 dark:bg-red-500/15 dark:border-red-400 text-red-900 dark:text-red-200'
                      : type === 'rest_day'
                        ? 'border-blue-400 bg-blue-50 dark:bg-blue-500/15 dark:border-blue-400 text-blue-900 dark:text-blue-200'
                        : 'border-gray-900 dark:border-slate-300 bg-gray-50 dark:bg-slate-800 text-gray-900 dark:text-slate-100'
                    : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600 text-gray-700 dark:text-slate-300'
                }`}
              >
                <input
                  type="radio"
                  name={`${formId}-ot_type`}
                  checked={form.ot_type === type}
                  onChange={() => setForm((prev) => ({ ...prev, ot_type: type }))}
                  className="sr-only"
                />
                <span className="text-xs font-medium">{t(`enums.otType.${type}`)}</span>
                <span className="text-[10px] text-gray-500 dark:text-slate-400">
                  {type === 'normal' ? '1.5x' : type === 'rest_day' ? '2.0x' : '3.0x'}
                </span>
              </label>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <TimeSelector
            label={`${t('common.startTime')} *`}
            value={form.start_time}
            onChange={(value) => updateTime('start_time', value)}
          />
          <TimeSelector
            label={`${t('common.endTime')} *`}
            value={form.end_time}
            onChange={(value) => updateTime('end_time', value)}
          />
        </div>

        {form.hours > 0 && (
          <div className="bg-gray-50 dark:bg-slate-800/70 rounded-lg p-3 text-center">
            <span className="text-2xl font-bold text-gray-900 dark:text-slate-100 tabular-nums">{form.hours}</span>
            <span className="text-sm text-gray-500 dark:text-slate-400 ml-1">{t('common.hours')}</span>
          </div>
        )}

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
      </form>
    </Modal>
  );
}
