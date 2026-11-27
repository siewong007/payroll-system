import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Clock, X, Trash2, AlertTriangle, Loader2 } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { EmptyState } from '@/components/ui/EmptyState';
import { FieldGroup } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { useAuth } from '@/context/AuthContext';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
import { getOvertimeApplications, createOvertimeApplication, cancelOvertimeApplication, deleteOvertimeApplication } from '@/api/portal';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';
import { formatDate, getErrorMessage } from '@/lib/utils';
import { OT_DEFAULT_END, OT_DEFAULT_HOURS, OT_DEFAULT_START, calculateOvertimeHours } from '@/lib/overtime';
import type { OvertimeApplication, CreateOvertimeRequest } from '@/types';
import i18n from '@/i18n';

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending',
    approved: 'badge-approved',
    rejected: 'badge-rejected',
    cancelled: 'badge-cancelled',
  };
  return (
    <span className={`badge ${cls[status] || 'badge-draft'}`}>
      {i18n.t(`enums.requestStatus.${status}`, { defaultValue: status })}
    </span>
  );
};

const otTypeLabel = (t: string) => i18n.t(`enums.otType.${t}`, { defaultValue: t });

const otTypeMultiplier = (t: string) => {
  const m: Record<string, string> = { normal: '1.5x', rest_day: '2.0x', public_holiday: '3.0x' };
  return m[t] || '';
};

const STATUS_FILTERS = [
  { key: null, labelKey: 'common.all' },
  { key: 'pending', labelKey: 'enums.requestStatus.pending' },
  { key: 'approved', labelKey: 'enums.requestStatus.approved' },
  { key: 'rejected', labelKey: 'enums.requestStatus.rejected' },
  { key: 'cancelled', labelKey: 'enums.requestStatus.cancelled' },
] as const;

export function Overtime() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const hasEmployee = Boolean(user?.employee_id);
  const confirmDialog = useConfirm();
  const toast = useToast();
  const queryClient = useQueryClient();
  const selectAllRef = useRef<HTMLInputElement>(null);
  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [selectedOvertimeIds, setSelectedOvertimeIds] = useState<string[]>([]);
  const [formError, setFormError] = useState('');
  const [bulkError, setBulkError] = useState('');
  // Seeded with the times the fields display. TimeSelector has no empty option,
  // so state that holds '' behind a rendered 18:00 is a form fighting its own
  // screen — that is what left Submit permanently greyed with no explanation.
  const [form, setForm] = useState<CreateOvertimeRequest>({
    ot_date: '',
    start_time: OT_DEFAULT_START,
    end_time: OT_DEFAULT_END,
    hours: OT_DEFAULT_HOURS,
    ot_type: 'normal',
    reason: '',
  });

  const { data: applications = [], isLoading, isError, error, refetch } = useQuery({
    queryKey: ['my-overtime'],
    queryFn: getOvertimeApplications,
    enabled: hasEmployee,
  });

  const createMutation = useMutation({
    mutationFn: createOvertimeApplication,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['my-overtime'] });
      setShowModal(false);
      resetForm();
    },
  });

  const cancelMutation = useMutation({
    mutationFn: cancelOvertimeApplication,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-overtime'] }),
    onError: (err) => toast.error(getErrorMessage(err, t('portal.overtime.bulkCancelFailed'))),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteOvertimeApplication,
    onSuccess: (_data, deletedId) => {
      setSelectedOvertimeIds((prev) => prev.filter((id) => id !== deletedId));
      queryClient.invalidateQueries({ queryKey: ['my-overtime'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('portal.overtime.bulkDeleteFailed'))),
  });

  // Partial failure is an outcome, not an exception: the refetch runs whatever
  // happens and only the applications that failed stay selected, so a retry hits
  // exactly those. `Promise.all` used to short-circuit on the first rejection
  // and leave the successful rows showing a stale status with no error anywhere.
  const bulkCancelMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, cancelOvertimeApplication),
    onSuccess: (outcome) => {
      setSelectedOvertimeIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'cancelled'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.overtime.bulkCancelFailed'))),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['my-overtime'] }),
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, deleteOvertimeApplication),
    onSuccess: (outcome) => {
      setSelectedOvertimeIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'deleted'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.overtime.bulkDeleteFailed'))),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['my-overtime'] }),
  });

  // The OT date stays empty on purpose: with the times defaulted, it is the one
  // deliberate input standing between an open modal and a submitted record.
  const resetForm = () => {
    setFormError('');
    createMutation.reset();
    setForm({
      ot_date: '',
      start_time: OT_DEFAULT_START,
      end_time: OT_DEFAULT_END,
      hours: OT_DEFAULT_HOURS,
      ot_type: 'normal',
      reason: '',
    });
  };

  const updateTime = (field: 'start_time' | 'end_time', value: string) => {
    const updated = { ...form, [field]: value };
    updated.hours = calculateOvertimeHours(
      field === 'start_time' ? value : form.start_time,
      field === 'end_time' ? value : form.end_time,
    );
    setForm(updated);
  };

  const handleSubmit = () => {
    setFormError('');
    if (!form.ot_date) {
      setFormError(t('portal.overtime.dateRequired'));
      return;
    }
    if (!form.start_time || !form.end_time || form.hours <= 0) {
      setFormError(t('portal.overtime.timesMustDiffer'));
      return;
    }
    createMutation.mutate({
      ...form,
      reason: form.reason || undefined,
    });
  };

  const handleCancel = async (app: OvertimeApplication) => {
    const ok = await confirmDialog({
      description: t('portal.overtime.cancelConfirm'),
      confirmLabel: t('portal.overtime.cancelApplication'),
      variant: 'warning',
    });
    if (ok) {
      cancelMutation.mutate(app.id);
    }
  };

  const handleDelete = async (app: OvertimeApplication) => {
    const ok = await confirmDialog({
      description: t('portal.overtime.deleteConfirm'),
      variant: 'danger',
    });
    if (ok) {
      deleteMutation.mutate(app.id);
    }
  };

  const canCancel = (app: OvertimeApplication) => ['pending', 'approved', 'rejected'].includes(app.status);
  const filteredApplications = useMemo(
    () => (statusFilter ? applications.filter((app) => app.status === statusFilter) : applications),
    [applications, statusFilter],
  );
  const displayedOvertimeIds = filteredApplications.map((app) => app.id);
  const selectedDisplayedIds = selectedOvertimeIds.filter((id) => displayedOvertimeIds.includes(id));
  const allDisplayedSelected = displayedOvertimeIds.length > 0 && selectedDisplayedIds.length === displayedOvertimeIds.length;
  const someDisplayedSelected = selectedDisplayedIds.length > 0 && !allDisplayedSelected;
  const selectedApplications = applications.filter((app) => selectedOvertimeIds.includes(app.id));
  const selectedCancelableIds = selectedApplications.filter(canCancel).map((app) => app.id);
  const selectedDeletableIds = selectedApplications.filter((app) => app.status === 'cancelled').map((app) => app.id);

  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = someDisplayedSelected;
    }
  }, [someDisplayedSelected]);

  const toggleOvertimeSelection = (id: string) => {
    setSelectedOvertimeIds((current) => (
      current.includes(id) ? current.filter((selectedId) => selectedId !== id) : [...current, id]
    ));
  };

  const toggleAllDisplayedOvertime = () => {
    setSelectedOvertimeIds((current) => (
      allDisplayedSelected
        ? current.filter((id) => !displayedOvertimeIds.includes(id))
        : Array.from(new Set([...current, ...displayedOvertimeIds]))
    ));
  };

  if (!hasEmployee) {
    return (
      <div className="space-y-6">
        <PageHeader className="mb-0" title={t('portal.overtime.title')} subtitle={t('portal.overtime.subtitle')} />
        <div className="card">
          <EmptyState icon={Clock} title={t('attendance.scan.noEmployeeProfile')} />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        className="mb-0"
        title={t('portal.overtime.title')}
        subtitle={t('portal.overtime.subtitle')}
        actions={
          <button
            type="button"
            onClick={() => { resetForm(); setShowModal(true); }}
            className="btn-primary w-full sm:w-auto"
          >
            <Plus className="w-4 h-4" aria-hidden="true" />
            {t('portal.overtime.apply')}
          </button>
        }
      />

      {/* Status Filter Pills */}
      <div className="flex gap-2 overflow-x-auto pb-1 sm:flex-wrap sm:overflow-visible sm:pb-0 scrollbar-thin">
        {STATUS_FILTERS.map((tab) => {
          const isActive = statusFilter === tab.key;
          const count = tab.key ? applications.filter((a) => a.status === tab.key).length : applications.length;
          return (
            <button
              key={tab.key ?? 'all'}
              type="button"
              aria-pressed={isActive}
              onClick={() => {
                setSelectedOvertimeIds([]);
                setBulkError('');
                setStatusFilter(tab.key);
              }}
              className={`shrink-0 min-h-11 sm:min-h-9 px-4 py-1.5 rounded-full text-sm font-medium transition-all-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                isActive
                  ? 'bg-[var(--accent-1)] text-white'
                  : 'bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-300 border border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700 hover:text-gray-900 dark:hover:text-white'
              }`}
            >
              {t(tab.labelKey)} {count > 0 && <span className="ml-1 opacity-80 tabular-nums">{count}</span>}
            </button>
          );
        })}
      </div>

      {/* Applications List */}
      <div className="card !p-0 overflow-hidden">
        <div className="flex items-center justify-between gap-3 px-4 sm:px-6 py-4 border-b border-gray-100 dark:border-slate-800">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('portal.overtime.myApplications')}</h2>
          {filteredApplications.length > 0 && (
            <label className="min-h-11 sm:min-h-9 inline-flex items-center gap-2 text-xs font-medium text-gray-500 dark:text-slate-400 cursor-pointer">
              <input
                ref={selectAllRef}
                type="checkbox"
                checked={allDisplayedSelected}
                onChange={toggleAllDisplayedOvertime}
                className="h-4 w-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)] focus:ring-[var(--ring)]"
              />
              {t('common.selectAll')}
            </label>
          )}
        </div>
        {selectedOvertimeIds.length > 0 && (
          <div className="border-b border-gray-100 dark:border-slate-800 bg-gray-50 dark:bg-slate-800/60">
            <div className="flex flex-col gap-2 px-4 sm:px-6 py-3 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-sm font-medium text-gray-700 dark:text-slate-200 tabular-nums">{t('common.selected', { count: selectedOvertimeIds.length })}</span>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.overtime.bulkCancelConfirm', { count: selectedCancelableIds.length }),
                      confirmLabel: t('common.cancelSelected'),
                      variant: 'warning',
                    });
                    if (ok) {
                      setBulkError('');
                      bulkCancelMutation.mutate(selectedCancelableIds);
                    }
                  }}
                  disabled={selectedCancelableIds.length === 0 || bulkCancelMutation.isPending}
                  className="btn-secondary min-h-11 sm:!py-2 text-sm disabled:opacity-50"
                >
                  <X className="w-4 h-4" aria-hidden="true" />
                  {bulkCancelMutation.isPending ? t('common.cancelling') : t('common.cancelSelected')}
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.overtime.bulkDeleteConfirm', { count: selectedDeletableIds.length }),
                      variant: 'danger',
                    });
                    if (ok) {
                      setBulkError('');
                      bulkDeleteMutation.mutate(selectedDeletableIds);
                    }
                  }}
                  disabled={selectedDeletableIds.length === 0 || bulkDeleteMutation.isPending}
                  className="btn-secondary min-h-11 sm:!py-2 text-sm text-red-600 dark:text-red-400 hover:!bg-red-50 dark:hover:!bg-red-500/10 disabled:opacity-50"
                >
                  <Trash2 className="w-4 h-4" aria-hidden="true" />
                  {bulkDeleteMutation.isPending ? t('common.deleting') : t('common.deleteSelected')}
                </button>
              </div>
            </div>
            {bulkError && (
              <div role="alert" className="px-4 sm:px-6 pb-3 text-sm text-red-700 dark:text-red-300">{bulkError}</div>
            )}
          </div>
        )}
        {isLoading ? (
          <div role="status" aria-busy="true" aria-label={t('common.loading')} className="p-4 sm:p-6 space-y-3">
            {[0, 1, 2].map((i) => (
              <Skeleton key={i} className="h-20 w-full rounded-xl" />
            ))}
          </div>
        ) : isError ? (
          <div className="p-6 sm:p-8">
            <EmptyState
              icon={AlertTriangle}
              title={t('portal.overtime.loadFailed')}
              description={getErrorMessage(error, t('portal.overtime.loadFailed'))}
              action={
                <button type="button" onClick={() => void refetch()} className="btn-primary">
                  {t('common.retry')}
                </button>
              }
            />
          </div>
        ) : filteredApplications.length === 0 ? (
          <div className="p-6">
            <EmptyState
              icon={Clock}
              title={t('portal.overtime.empty')}
              action={
                statusFilter ? (
                  <button type="button" onClick={() => setStatusFilter(null)} className="btn-secondary">
                    {t('ux.table.clearFilters')}
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => { resetForm(); setShowModal(true); }}
                    className="btn-primary"
                  >
                    <Plus className="w-4 h-4" aria-hidden="true" />
                    {t('portal.overtime.apply')}
                  </button>
                )
              }
            />
          </div>
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-slate-800">
            {filteredApplications.map((app) => (
              <div key={app.id} className="px-4 sm:px-6 py-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-start gap-2 sm:gap-3 min-w-0">
                    <label className="-m-2 inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center cursor-pointer">
                      <input
                        type="checkbox"
                        checked={selectedOvertimeIds.includes(app.id)}
                        onChange={() => toggleOvertimeSelection(app.id)}
                        className="h-4 w-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)] focus:ring-[var(--ring)]"
                        aria-label={t('portal.overtime.selectAria', { date: formatDate(app.ot_date) })}
                      />
                    </label>
                    <div className="space-y-1 min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-semibold text-gray-900 dark:text-slate-100 tabular-nums">{formatDate(app.ot_date)}</span>
                        {statusBadge(app.status)}
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium tabular-nums ${
                          app.ot_type === 'public_holiday' ? 'bg-red-100 dark:bg-red-500/20 text-red-700 dark:text-red-300' :
                          app.ot_type === 'rest_day' ? 'bg-blue-100 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300' :
                          'bg-gray-100 dark:bg-slate-800 text-gray-600 dark:text-slate-300'
                        }`}>
                          {otTypeLabel(app.ot_type)} ({otTypeMultiplier(app.ot_type)})
                        </span>
                      </div>
                      <p className="text-sm text-gray-500 dark:text-slate-400 tabular-nums">
                        {app.start_time?.slice(0, 5)} — {app.end_time?.slice(0, 5)}
                        <span className="ml-2 font-medium text-gray-700 dark:text-slate-200">{t('attendance.hoursShort', { value: Number(app.hours) })}</span>
                      </p>
                      {app.reason && <p className="text-sm text-gray-500 dark:text-slate-400">{app.reason}</p>}
                      {app.review_notes && (
                        <p className="text-sm text-amber-600 dark:text-amber-400 mt-1">{t('portal.overtime.reviewNote', { notes: app.review_notes })}</p>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-1 shrink-0">
                    {canCancel(app) && (
                      <button
                        type="button"
                        onClick={() => handleCancel(app)}
                        disabled={cancelMutation.isPending}
                        aria-label={`${t('portal.overtime.cancelApplication')} — ${formatDate(app.ot_date)}`}
                        className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                        title={t('portal.overtime.cancelApplication')}
                      >
                        <X className="w-4 h-4" aria-hidden="true" />
                      </button>
                    )}
                    {app.status === 'cancelled' && (
                      <button
                        type="button"
                        onClick={() => handleDelete(app)}
                        disabled={deleteMutation.isPending}
                        aria-label={`${t('common.deletePermanently')} — ${formatDate(app.ot_date)}`}
                        className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-red-500 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                        title={t('common.deletePermanently')}
                      >
                        <Trash2 className="w-4 h-4" aria-hidden="true" />
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Create Modal */}
      <Modal
        open={showModal}
        onClose={() => { if (!createMutation.isPending) setShowModal(false); }}
        title={t('portal.overtime.modalTitle')}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button
              type="button"
              onClick={() => setShowModal(false)}
              disabled={createMutation.isPending}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form="overtime-create-form"
              disabled={createMutation.isPending}
              className="btn-primary"
            >
              {createMutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
              {createMutation.isPending ? t('common.submitting') : t('portal.overtime.submit')}
            </button>
          </div>
        }
      >
        <form
          id="overtime-create-form"
          onSubmit={(e) => {
            e.preventDefault();
            handleSubmit();
          }}
          className="space-y-4"
        >
          <div>
            <label className="form-label" htmlFor="ot-date">{t('portal.overtime.otDate')} *</label>
            <input
              id="ot-date"
              type="date"
              required
              value={form.ot_date}
              onChange={(e) => setForm({ ...form, ot_date: e.target.value })}
              className="form-input tabular-nums"
            />
          </div>

          <FieldGroup legend={t('portal.overtime.otType')} required>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
              {(['normal', 'rest_day', 'public_holiday'] as const).map((otType) => (
                <label
                  key={otType}
                  className={`min-h-11 flex sm:flex-col items-center justify-between sm:justify-center gap-1 px-3 py-3 rounded-lg border-2 cursor-pointer transition-all text-center focus-within:ring-2 focus-within:ring-[var(--ring)] focus-within:ring-offset-1 ${
                    form.ot_type === otType
                      ? otType === 'public_holiday'
                        ? 'border-red-400 dark:border-red-500 bg-red-50 dark:bg-red-500/15 text-red-900 dark:text-red-200'
                        : otType === 'rest_day'
                          ? 'border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-500/15 text-blue-900 dark:text-blue-200'
                          : 'border-[var(--accent-1)] bg-[var(--accent-soft)] text-gray-900 dark:text-slate-100'
                      : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600 text-gray-700 dark:text-slate-200'
                  }`}
                >
                  <input
                    type="radio"
                    name="ot_type"
                    value={otType}
                    checked={form.ot_type === otType}
                    onChange={() => setForm({ ...form, ot_type: otType })}
                    className="sr-only"
                  />
                  <span className="text-xs font-medium">{otTypeLabel(otType)}</span>
                  <span className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">{otTypeMultiplier(otType)}</span>
                </label>
              ))}
            </div>
          </FieldGroup>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <TimeSelector
              label={t('portal.overtime.startTime').replace(/\s*\*$/, '')}
              required
              value={form.start_time}
              onChange={(value) => updateTime('start_time', value)}
            />
            <TimeSelector
              label={t('portal.overtime.endTime').replace(/\s*\*$/, '')}
              required
              value={form.end_time}
              onChange={(value) => updateTime('end_time', value)}
            />
          </div>

          <div className={`rounded-lg p-3 text-center ${form.hours > 0 ? 'bg-gray-50 dark:bg-slate-800/80' : 'bg-amber-50 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/30'}`}>
            {form.hours > 0 ? (
              <>
                <span className="text-2xl font-bold text-gray-900 dark:text-slate-100 tabular-nums">{form.hours}</span>
                <span className="text-sm text-gray-500 dark:text-slate-400 ml-1">{t('portal.overtime.hoursUnit')}</span>
              </>
            ) : (
              <span className="text-sm font-medium text-amber-800 dark:text-amber-300">{t('portal.overtime.timesMustDiffer')}</span>
            )}
          </div>

          <div>
            <label className="form-label" htmlFor="ot-reason">{t('common.reason')}</label>
            <textarea
              id="ot-reason"
              value={form.reason}
              onChange={(e) => setForm({ ...form, reason: e.target.value })}
              className="form-input"
              rows={2}
              placeholder={t('portal.overtime.reasonPlaceholder')}
            />
          </div>

          {formError && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{formError}</p>}

          {createMutation.isError && (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">{getErrorMessage(createMutation.error, t('portal.overtime.submitFailed'))}</p>
          )}
        </form>
      </Modal>
    </div>
  );
}
