import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle,
  AlertTriangle,
  Camera,
  Loader2,
  LogIn,
  LogOut,
  MapPin,
  MoreVertical,
  Pencil,
  ShieldAlert,
  Smartphone,
  Timer,
  WifiOff,
  X,
} from 'lucide-react';
import {
  createManualAttendance,
  updateAttendanceRecord,
  type AttendanceRecordWithEmployee,
} from '@/api/attendance';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { zonedDateKey } from '@/lib/attendance';
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
  const formId = useId();
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
      void queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
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
    const checkInDate = form.check_in_at ? new Date(form.check_in_at) : null;
    if (!checkInDate || Number.isNaN(checkInDate.getTime())) {
      setError(t('attendance.edit.checkInRequired'));
      return;
    }
    const checkOutDate = form.check_out_at ? new Date(form.check_out_at) : null;
    if (checkOutDate && Number.isNaN(checkOutDate.getTime())) {
      setError(t('attendance.manual.createFailed'));
      return;
    }
    setError('');
    mutation.mutate({
      employee_id: form.employee_id.trim(),
      check_in_at: checkInDate.toISOString(),
      check_out_at: checkOutDate ? checkOutDate.toISOString() : undefined,
      status: form.status,
      notes: form.notes || undefined,
    });
  };

  return (
    <Modal
      open
      onClose={() => {
        if (!mutation.isPending) onClose();
      }}
      title={t('attendance.manual.title')}
      maxWidth="max-w-md"
      footer={
        <div className="flex flex-col-reverse sm:flex-row sm:justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={mutation.isPending}
            className="btn-secondary"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={mutation.isPending}
            className="btn-primary inline-flex items-center justify-center gap-2"
          >
            {mutation.isPending ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                {t('common.saving')}
              </>
            ) : (
              t('common.save')
            )}
          </button>
        </div>
      }
    >
      <form id={formId} onSubmit={handleSubmit} className="space-y-4">
        <FormField label={t('common.employee')} required>
          {() =>
            presetEmployee ? (
              <div className="px-3 py-2 bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg text-sm text-gray-800 dark:text-slate-100">
                {presetEmployee.label}
              </div>
            ) : (
              <EmployeePicker
                value={form.employee_id}
                onChange={(id) => setForm((p) => ({ ...p, employee_id: id }))}
              />
            )
          }
        </FormField>
        <FormField label={t('attendance.manual.checkIn')} required>
          {(aria) => (
            <input
              {...aria}
              type="datetime-local"
              required
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
              min={form.check_in_at || undefined}
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
          <p role="alert" className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
            {error}
          </p>
        )}
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
  const formId = useId();
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
        clear_check_out: hadCheckOut && !form.check_out_at ? true : undefined,
        clear_notes: hadNotes && !form.notes ? true : undefined,
        reason: form.reason.trim(),
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.edit.updateFailed'))),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const checkInDate = form.check_in_at ? new Date(form.check_in_at) : null;
    if (!checkInDate || Number.isNaN(checkInDate.getTime())) {
      setError(t('attendance.edit.checkInRequired'));
      return;
    }
    if (!form.reason.trim()) {
      setError(t('attendance.edit.reasonRequired'));
      return;
    }
    setError('');
    mutation.mutate();
  };

  return (
    <Modal
      open
      onClose={() => {
        if (!mutation.isPending) {
          onClose();
        }
      }}
      title={t('attendance.edit.title')}
      maxWidth="max-w-md"
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={mutation.isPending}
            className="btn-secondary w-full sm:w-auto"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={mutation.isPending}
            className="btn-primary w-full sm:w-auto"
          >
            {mutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
            {mutation.isPending ? t('common.saving') : t('common.saveChanges')}
          </button>
        </div>
      }
    >
      <p className="text-sm text-gray-500 dark:text-slate-400 -mt-2 mb-4">
        {record.full_name} ({record.employee_number})
      </p>
      <form id={formId} onSubmit={handleSubmit} className="space-y-4">
        <FormField label={t('attendance.manual.checkIn')} required>
          {(aria) => (
            <input
              {...aria}
              type="datetime-local"
              required
              value={form.check_in_at}
              onChange={(e) => setForm((p) => ({ ...p, check_in_at: e.target.value }))}
              className="form-input"
            />
          )}
        </FormField>
        <div>
          <FormField label={t('attendance.manual.checkOut')}>
            {(aria) => (
              <div className="flex items-center gap-2">
                <input
                  {...aria}
                  type="datetime-local"
                  value={form.check_out_at}
                  min={form.check_in_at || undefined}
                  onChange={(e) => setForm((p) => ({ ...p, check_out_at: e.target.value }))}
                  className="form-input flex-1 min-w-0"
                />
                {form.check_out_at && (
                  <button
                    type="button"
                    onClick={() => setForm((p) => ({ ...p, check_out_at: '' }))}
                    className="btn-secondary min-h-11 sm:min-h-9 !py-2 !px-2.5 text-xs shrink-0"
                    title={t('attendance.edit.clearCheckOut')}
                    aria-label={t('attendance.edit.clearCheckOut')}
                  >
                    <X className="w-3.5 h-3.5" aria-hidden="true" />
                    <span className="hidden sm:inline">{t('attendance.edit.clearCheckOut')}</span>
                  </button>
                )}
              </div>
            )}
          </FormField>
          {hadCheckOut && !form.check_out_at && (
            <p className="mt-1 text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1">
              <AlertTriangle className="w-3 h-3 shrink-0" aria-hidden="true" />
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
              required
              value={form.reason}
              onChange={(e) => setForm((p) => ({ ...p, reason: e.target.value }))}
              placeholder={t('attendance.edit.reasonPlaceholder')}
              maxLength={400}
              className="form-input"
            />
          )}
        </FormField>
        {error && (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1.5">
            <AlertCircle className="w-4 h-4 shrink-0" aria-hidden="true" />
            {error}
          </p>
        )}
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
  onViewPhotos,
  timeZone = 'Asia/Kuala_Lumpur',
}: {
  record: AttendanceRecordWithEmployee;
  canEdit: boolean;
  onEdit: () => void;
  onViewPhotos?: () => void;
  timeZone?: string;
}) {
  const { t } = useTranslation();
  const statusCfg = STATUS_COLORS[r.status as keyof typeof STATUS_COLORS];
  const methodCfg = METHOD_CONFIG[r.method as keyof typeof METHOD_CONFIG];
  const MethodIcon = methodCfg?.icon ?? MoreVertical;
  const isAbsentPlaceholder = r.status === 'absent';
  const overnightOut =
    !isAbsentPlaceholder &&
    Boolean(r.check_out_at) &&
    zonedDateKey(r.check_out_at as string, timeZone) > zonedDateKey(r.check_in_at, timeZone);
  const hasPhotoInfo = Boolean(
    r.check_in_photo_url || r.check_out_photo_url || r.check_in_photo_purged_at || r.check_out_photo_purged_at,
  );

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-medium text-gray-900 dark:text-slate-100 line-clamp-2">{r.full_name}</p>
          <p className="text-xs text-gray-500 dark:text-slate-400 truncate">
            {r.employee_number} · {formatDate(r.check_in_at, timeZone)}
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-1.5 shrink-0">
          {statusCfg && (
            <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg}`}>
              {attStatusLabel(r.status)}
            </span>
          )}
          {r.is_outside_geofence && (
            <span
              title={t('attendance.records.outsideGeofence')}
              className="inline-flex items-center gap-1 rounded-full bg-amber-50 dark:bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300"
            >
              <AlertTriangle className="w-3.5 h-3.5" aria-hidden="true" />
              <span>{t('attendance.records.outsideGeofence')}</span>
            </span>
          )}
          {r.is_offsite_network && (
            <span
              title={t('attendance.records.offsiteNetwork')}
              className="inline-flex items-center gap-1 rounded-full bg-amber-50 dark:bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300"
            >
              <WifiOff className="w-3.5 h-3.5" aria-hidden="true" />
              <span>{t('attendance.records.offsiteNetwork')}</span>
            </span>
          )}
          {r.anomaly_flags?.map((flag) => (
            <span
              key={flag}
              className="inline-flex items-center gap-1 rounded-full bg-red-50 dark:bg-red-500/15 px-2 py-0.5 text-xs font-medium text-red-700 dark:text-red-300"
            >
              <ShieldAlert className="w-3 h-3" aria-hidden="true" />
              <span>{t(`attendance.anomalies.flags.${flag}`, { defaultValue: flag })}</span>
            </span>
          ))}
          {canEdit && (
            <button
              type="button"
              onClick={onEdit}
              className="-mr-1 min-h-11 min-w-11 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
              title={t('attendance.records.editRecord')}
              aria-label={`${t('attendance.records.editRecord')} \u2014 ${r.full_name}`}
            >
              <Pencil className="w-4 h-4" aria-hidden="true" />
            </button>
          )}
        </div>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-3">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400">
            {t('attendance.records.checkIn')}
          </p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700 dark:text-slate-200">
            <LogIn className="w-3.5 h-3.5 shrink-0 text-emerald-500 dark:text-emerald-400" aria-hidden="true" />
            {isAbsentPlaceholder ? '—' : formatTime(r.check_in_at, timeZone)}
          </p>
        </div>
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400">
            {t('attendance.records.checkOut')}
          </p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700 dark:text-slate-200">
            <LogOut className="w-3.5 h-3.5 shrink-0 text-gray-500 dark:text-slate-400" aria-hidden="true" />
            {isAbsentPlaceholder ? '—' : formatTime(r.check_out_at, timeZone)}
            {overnightOut && (
              <span className="rounded bg-gray-100 dark:bg-slate-800 px-1 py-px text-[10px] font-medium text-gray-600 dark:text-slate-300">
                {t('attendance.records.nextDay')}
              </span>
            )}
          </p>
        </div>
      </div>

      {r.notes && (
        <p className="mt-2 text-xs text-gray-500 dark:text-slate-400 line-clamp-2">{r.notes}</p>
      )}

      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs">
        <span className="tabular-nums text-gray-600 dark:text-slate-300">
          {r.hours_worked != null
            ? t('attendance.hoursShort', { value: Number(r.hours_worked).toFixed(1) })
            : '—'}
          {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
            <span className="ml-1 text-amber-600 dark:text-amber-400 font-medium">
              <Timer className="w-3 h-3 inline" aria-hidden="true" />{' '}
              {t('attendance.hoursOtShort', { value: Number(r.overtime_hours).toFixed(1) })}
            </span>
          )}
        </span>
        <span className={`flex items-center gap-1.5 ${methodCfg?.color ?? 'text-gray-500 dark:text-slate-400'}`}>
          <MethodIcon className="w-3.5 h-3.5" aria-hidden="true" />
          <span className="font-medium">{methodCfg ? attMethodLabel(r.method) : r.method}</span>
        </span>
        {r.latitude && r.longitude && (
          <a
            href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
            target="_blank"
            rel="noopener noreferrer"
            className="min-h-9 inline-flex items-center gap-1 text-sky-600 dark:text-sky-400 hover:text-sky-700 dark:hover:text-sky-300 hover:underline"
          >
            <MapPin className="w-3.5 h-3.5" aria-hidden="true" />
            {t('attendance.records.checkInLocation')}
          </a>
        )}
        {r.checkout_latitude && r.checkout_longitude && (
          <a
            href={`https://maps.google.com/?q=${r.checkout_latitude},${r.checkout_longitude}`}
            target="_blank"
            rel="noopener noreferrer"
            className="min-h-9 inline-flex items-center gap-1 text-[var(--accent-1)] hover:underline"
          >
            <MapPin className="w-3.5 h-3.5" aria-hidden="true" />
            {t('attendance.records.checkOutLocation')}
          </a>
        )}
        {r.check_in_device_label && (
          <span className="inline-flex items-center gap-1 text-gray-500 dark:text-slate-400">
            <Smartphone className="w-3 h-3" aria-hidden="true" />
            {r.check_in_device_label}
          </span>
        )}
        {hasPhotoInfo && onViewPhotos && (
          <button
            type="button"
            onClick={onViewPhotos}
            className="inline-flex items-center gap-1 text-[var(--accent-1)] hover:underline font-medium"
          >
            <Camera className="w-3.5 h-3.5" aria-hidden="true" />
            {t('attendance.photo.viewPhotos')}
          </button>
        )}
      </div>
    </div>
  );
}

export function AttendancePhotoModal({
  record,
  onClose,
}: {
  record: AttendanceRecordWithEmployee;
  onClose: () => void;
}) {
  const { t } = useTranslation();

  return (
    <Modal
      open
      onClose={onClose}
      title={`${t('attendance.photo.modalTitle')} — ${record.full_name}`}
      maxWidth="max-w-lg"
      footer={
        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="btn-primary">
            {t('common.close')}
          </button>
        </div>
      }
    >
      <div className="space-y-5">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 mb-2">
            {t('attendance.records.checkIn')}
          </p>
          {record.check_in_photo_url ? (
            <AttachmentPreview url={record.check_in_photo_url} name={t('attendance.photo.checkInTitle')} />
          ) : record.check_in_photo_purged_at ? (
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('attendance.photo.purgedNotice')}
            </p>
          ) : (
            <p className="text-sm text-gray-400 dark:text-slate-500">—</p>
          )}
        </div>

        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 mb-2">
            {t('attendance.records.checkOut')}
          </p>
          {record.check_out_photo_url ? (
            <AttachmentPreview url={record.check_out_photo_url} name={t('attendance.photo.checkOutTitle')} />
          ) : record.check_out_photo_purged_at ? (
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('attendance.photo.purgedNotice')}
            </p>
          ) : (
            <p className="text-sm text-gray-400 dark:text-slate-500">—</p>
          )}
        </div>
      </div>
    </Modal>
  );
}
