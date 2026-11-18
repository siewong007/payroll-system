import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle,
  AlertTriangle,
  LogIn,
  LogOut,
  MapPin,
  MoreVertical,
  Pencil,
  Timer,
} from 'lucide-react';
import {
  createManualAttendance,
  updateAttendanceRecord,
  type AttendanceRecordWithEmployee,
} from '@/api/attendance';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { getErrorMessage, toDateTimeLocalValue } from '@/lib/utils';
import {
  METHOD_CONFIG,
  STATUS_COLORS,
  attMethodLabel,
  attStatusLabel,
  formatDate,
  formatTime,
} from './attendanceHelpers';

export function ManualEntryModal({
  onClose,
  presetEmployee,
}: {
  onClose: () => void;
  presetEmployee?: { id: string; label: string };
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    employee_id: presetEmployee?.id ?? '',
    check_in_at: toDateTimeLocalValue(new Date()),
    check_out_at: '',
    status: 'present',
    notes: '',
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createManualAttendance,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.manual.createFailed'))),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.employee_id.trim()) {
      setError(t('attendance.manual.employeeRequired'));
      return;
    }
    mutation.mutate({
      employee_id: form.employee_id.trim(),
      check_in_at: new Date(form.check_in_at).toISOString(),
      check_out_at: form.check_out_at ? new Date(form.check_out_at).toISOString() : undefined,
      status: form.status,
      notes: form.notes || undefined,
    });
  };

  return (
    <Modal open onClose={onClose} title={t('attendance.manual.title')} maxWidth="max-w-md">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <span className="form-label">{t('common.employee')}</span>
          {presetEmployee ? (
            <div className="px-3 py-2 bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg text-sm text-gray-800 dark:text-slate-100">
              {presetEmployee.label}
            </div>
          ) : (
            <EmployeePicker
              value={form.employee_id}
              onChange={(id) => setForm((p) => ({ ...p, employee_id: id }))}
            />
          )}
        </div>
        <FormField label={t('attendance.manual.checkIn')}>
          {(aria) => (
            <input
              {...aria}
              type="datetime-local"
              value={form.check_in_at}
              onChange={(e) => setForm((p) => ({ ...p, check_in_at: e.target.value }))}
              className="form-input"
            />
          )}
        </FormField>
        <FormField label={t('attendance.manual.checkOutOptional')}>
          {(aria) => (
            <input
              {...aria}
              type="datetime-local"
              value={form.check_out_at}
              onChange={(e) => setForm((p) => ({ ...p, check_out_at: e.target.value }))}
              className="form-input"
            />
          )}
        </FormField>
        <FormField label={t('common.status')}>
          {(aria) => (
            <select
              {...aria}
              value={form.status}
              onChange={(e) =>
                setForm((p) => ({ ...p, status: e.target.value as keyof typeof STATUS_COLORS }))
              }
              className="form-input"
            >
              <option value="present">{attStatusLabel('present')}</option>
              <option value="late">{attStatusLabel('late')}</option>
              <option value="absent">{attStatusLabel('absent')}</option>
              <option value="half_day">{attStatusLabel('half_day')}</option>
            </select>
          )}
        </FormField>
        <FormField label={t('attendance.manual.notesOptional')}>
          {(aria) => (
            <textarea
              {...aria}
              value={form.notes}
              onChange={(e) => setForm((p) => ({ ...p, notes: e.target.value }))}
              rows={2}
              className="form-input resize-none"
            />
          )}
        </FormField>
        {error && (
          <p className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5" />
            {error}
          </p>
        )}
        <div className="flex gap-3 pt-2">
          <button type="button" onClick={onClose} className="btn-secondary flex-1">
            {t('common.cancel')}
          </button>
          <button type="submit" disabled={mutation.isPending} className="btn-primary flex-1">
            {mutation.isPending ? t('common.saving') : t('common.save')}
          </button>
        </div>
      </form>
    </Modal>
  );
}

export function EditAttendanceModal({
  record,
  onClose,
}: {
  record: AttendanceRecordWithEmployee;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    check_in_at: toDateTimeLocalValue(record.check_in_at),
    check_out_at: record.check_out_at ? toDateTimeLocalValue(record.check_out_at) : '',
    status: record.status,
    notes: record.notes ?? '',
    reason: '',
  });
  const [error, setError] = useState('');

  const hadCheckOut = record.check_out_at != null;
  const hadNotes = (record.notes ?? '') !== '';

  const mutation = useMutation({
    mutationFn: () =>
      updateAttendanceRecord(record.id, {
        check_in_at: new Date(form.check_in_at).toISOString(),
        check_out_at: form.check_out_at ? new Date(form.check_out_at).toISOString() : undefined,
        status: form.status,
        notes: form.notes || undefined,
        // Emptying a field previously did nothing: the server read a missing
        // value as "keep existing", so the record silently kept its old
        // check-out (and its payroll-relevant hours). Say "clear" explicitly.
        clear_check_out: hadCheckOut && !form.check_out_at ? true : undefined,
        clear_notes: hadNotes && !form.notes ? true : undefined,
        reason: form.reason.trim(),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.edit.updateFailed'))),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.reason.trim()) {
      setError(t('attendance.edit.reasonRequired'));
      return;
    }
    mutation.mutate();
  };

  return (
    <Modal open onClose={onClose} title={t('attendance.edit.title')} maxWidth="max-w-md">
      <p className="text-sm text-gray-500 dark:text-slate-400 -mt-2 mb-4">
        {record.full_name} ({record.employee_number})
      </p>
      <form onSubmit={handleSubmit} className="space-y-4">
        <FormField label={t('attendance.manual.checkIn')}>
          {(aria) => (
            <input
              {...aria}
              type="datetime-local"
              value={form.check_in_at}
              onChange={(e) => setForm((p) => ({ ...p, check_in_at: e.target.value }))}
              className="form-input"
            />
          )}
        </FormField>
        <div>
          <FormField label={t('attendance.manual.checkOut')}>
            {(aria) => (
              <input
                {...aria}
                type="datetime-local"
                value={form.check_out_at}
                onChange={(e) => setForm((p) => ({ ...p, check_out_at: e.target.value }))}
                className="form-input"
              />
            )}
          </FormField>
          {hadCheckOut && !form.check_out_at && (
            <p className="mt-1 text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1">
              <AlertTriangle className="w-3 h-3" />
              {t('attendance.edit.reopenWarning')}
            </p>
          )}
        </div>
        <FormField label={t('common.status')}>
          {(aria) => (
            <select
              {...aria}
              value={form.status}
              onChange={(e) =>
                setForm((p) => ({ ...p, status: e.target.value as keyof typeof STATUS_COLORS }))
              }
              className="form-input"
            >
              <option value="present">{attStatusLabel('present')}</option>
              <option value="late">{attStatusLabel('late')}</option>
              <option value="absent">{attStatusLabel('absent')}</option>
              <option value="half_day">{attStatusLabel('half_day')}</option>
            </select>
          )}
        </FormField>
        <FormField label={t('common.notes')}>
          {(aria) => (
            <textarea
              {...aria}
              value={form.notes}
              onChange={(e) => setForm((p) => ({ ...p, notes: e.target.value }))}
              rows={2}
              className="form-input resize-none"
            />
          )}
        </FormField>
        <FormField
          label={t('attendance.edit.reason')}
          required
          hint={t('attendance.edit.reasonNote')}
        >
          {(aria) => (
            <input
              {...aria}
              type="text"
              value={form.reason}
              onChange={(e) => setForm((p) => ({ ...p, reason: e.target.value }))}
              placeholder={t('attendance.edit.reasonPlaceholder')}
              maxLength={400}
              className="form-input"
            />
          )}
        </FormField>
        {error && (
          <p className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5" />
            {error}
          </p>
        )}
        <div className="flex gap-3 pt-2">
          <button type="button" onClick={onClose} className="btn-secondary flex-1">
            {t('common.cancel')}
          </button>
          <button type="submit" disabled={mutation.isPending} className="btn-primary flex-1">
            {mutation.isPending ? t('common.saving') : t('common.saveChanges')}
          </button>
        </div>
      </form>
    </Modal>
  );
}

/**
 * One attendance record as a card. The nine-column table needs ~1000px, so on
 * anything narrower it became a horizontal scroller where Status, Location and
 * the edit control sat off-screen — invisible unless you thought to swipe the
 * table sideways. Same fields, stacked.
 */
export function RecordCard({
  record: r,
  canEdit,
  onEdit,
}: {
  record: AttendanceRecordWithEmployee;
  canEdit: boolean;
  onEdit: () => void;
}) {
  const { t } = useTranslation();
  const statusCfg = STATUS_COLORS[r.status as keyof typeof STATUS_COLORS];
  const methodCfg = METHOD_CONFIG[r.method as keyof typeof METHOD_CONFIG];
  const MethodIcon = methodCfg?.icon ?? MoreVertical;

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          {/* Two lines, not `truncate` — this is the field you identify the
              record by, and Malaysian full names routinely exceed one line. */}
          <p className="font-medium text-gray-900 line-clamp-2">{r.full_name}</p>
          <p className="text-xs text-gray-400 truncate">
            {r.employee_number} · {formatDate(r.check_in_at)}
          </p>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          {statusCfg && (
            <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg}`}>
              {attStatusLabel(r.status)}
            </span>
          )}
          {r.is_outside_geofence && (
            <span title={t('attendance.records.outsideGeofence')} className="text-amber-500">
              <AlertTriangle className="w-4 h-4" />
            </span>
          )}
          {canEdit && (
            <button
              onClick={onEdit}
              className="-mr-1 p-2 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100 transition-colors"
              title={t('attendance.records.editRecord')}
            >
              <Pencil className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-3">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">
            {t('attendance.records.checkIn')}
          </p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700">
            <LogIn className="w-3.5 h-3.5 shrink-0 text-emerald-500" />
            {formatTime(r.check_in_at)}
          </p>
        </div>
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">
            {t('attendance.records.checkOut')}
          </p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700">
            <LogOut className="w-3.5 h-3.5 shrink-0 text-gray-400" />
            {formatTime(r.check_out_at)}
          </p>
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs">
        <span className="tabular-nums text-gray-600">
          {r.hours_worked != null
            ? t('attendance.hoursShort', { value: Number(r.hours_worked).toFixed(1) })
            : '—'}
          {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
            <span className="ml-1 text-amber-600 font-medium">
              <Timer className="w-3 h-3 inline" />{' '}
              {t('attendance.hoursOtShort', { value: Number(r.overtime_hours).toFixed(1) })}
            </span>
          )}
        </span>
        <span className={`flex items-center gap-1.5 ${methodCfg?.color ?? 'text-gray-500'}`}>
          <MethodIcon className="w-3.5 h-3.5" />
          <span className="font-medium">{methodCfg ? attMethodLabel(r.method) : r.method}</span>
        </span>
        {r.latitude && r.longitude && (
          <a
            href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 text-sky-600 hover:text-sky-700"
          >
            <MapPin className="w-3.5 h-3.5" />
            {t('attendance.records.location')}
          </a>
        )}
      </div>
    </div>
  );
}
