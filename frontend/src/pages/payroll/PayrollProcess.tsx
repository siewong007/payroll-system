import { useEffect, useId, useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Calculator, Lock, Pencil, Plus, RefreshCw, Trash2, X } from 'lucide-react';
import {
  createPayrollEntry,
  deletePayrollEntry,
  getPayrollEntries,
  getPayrollGroups,
  previewPayroll,
  processPayroll,
  updatePayrollEntry,
} from '@/api/payroll';
import { getJob } from '@/api/jobs';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { formatMYR, getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatPeriod, monthName } from '@/lib/format';
import { PayrollPreviewPanel } from './PayrollPreviewPanel';
import { PayrollLifecycleStepper } from './PayrollLifecycleStepper';
import { FormField } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import type { PayrollEntryWithEmployee } from '@/types';
import type { TFunction } from 'i18next';

const MONTH_VALUES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

type EntryKind = 'monthly_allowance' | 'other_earning' | 'deduction';

function getEntryKind(entry: Pick<PayrollEntryWithEmployee, 'category' | 'item_type'>): EntryKind {
  if (entry.category === 'earning' && ['allowance', 'monthly_allowance'].includes(entry.item_type)) {
    return 'monthly_allowance';
  }
  return entry.category === 'deduction' ? 'deduction' : 'other_earning';
}

function getEntryKindLabel(entry: Pick<PayrollEntryWithEmployee, 'category' | 'item_type'>, t: TFunction) {
  return t(`payroll.process.entryKinds.${getEntryKind(entry)}`);
}

function formatEntryItemType(itemType: string, t: TFunction): string {
  return t(`payroll.process.itemTypes.${itemType}`, { defaultValue: itemType });
}

export function PayrollProcess() {
  const { t } = useTranslation();
  const confirmDialog = useConfirm();
  const toast = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const now = new Date();
  const taxableCheckboxId = useId();
  const defaultDesc = t('payroll.process.defaultDescription');

  const [groupId, setGroupId] = useState('');
  const [year, setYear] = useState(now.getFullYear());
  const [month, setMonth] = useState(now.getMonth() + 1);
  const [notes, setNotes] = useState('');
  const [processError, setProcessError] = useState('');
  const [editingEntry, setEditingEntry] = useState<PayrollEntryWithEmployee | null>(null);
  const [entryError, setEntryError] = useState('');
  const [entryForm, setEntryForm] = useState({
    employee_id: '',
    // Carried alongside the id so editing an entry can show the employee's name
    // without the picker having to find them in a page of results.
    employee_label: '',
    category: 'earning' as 'earning' | 'deduction',
    item_type: 'monthly_allowance',
    description: defaultDesc,
    amount: '',
    quantity: '',
    rate: '',
    is_taxable: true,
  });

  const {
    data: groups,
    isError: groupsError,
    error: groupsErrorObj,
    refetch: refetchGroups,
  } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
  });

  const {
    data: entries = [],
    isError: entriesError,
    error: entriesErrorObj,
    refetch: refetchEntries,
  } = useQuery({
    queryKey: ['payrollEntries', year, month],
    queryFn: () => getPayrollEntries({ period_year: year, period_month: month }),
  });

  // Processing is now a two-step review: the preview holds the projected run so
  // the operator confirms figures rather than discovering them afterwards.
  const previewMutation = useMutation({
    mutationFn: previewPayroll,
    onError: (err: unknown) => {
      setProcessError(getErrorMessage(err, t('payroll.process.previewFailed')));
    },
    onSuccess: () => setProcessError(''),
  });

  // Processing is detached: submit returns a job, and this poll follows it to
  // the terminal state — `result.run_id` on success, `error` on failure.
  const [jobId, setJobId] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: processPayroll,
    onSuccess: (job) => {
      setProcessError('');
      setJobId(job.id);
    },
    onError: (err: unknown) => {
      setProcessError(getErrorMessage(err, t('payroll.process.processFailed')));
    },
  });

  const {
    data: processJob,
    isError: jobPollError,
    error: jobPollErrorObj,
    refetch: refetchJob,
  } = useQuery({
    queryKey: ['payrollProcessJob', jobId],
    queryFn: () => getJob(jobId as string),
    enabled: jobId !== null,
    refetchInterval: (query) =>
      query.state.status !== 'error' &&
      (query.state.data?.status === 'pending' || query.state.data?.status === 'running')
        ? 1_000
        : false,
  });

  const jobRunning =
    jobId !== null &&
    processJob?.status !== 'succeeded' &&
    processJob?.status !== 'failed';

  const preview = previewMutation.data;
  const adjustmentsLocked = Boolean(preview) || jobRunning;

  const entryDirty = Boolean(
    editingEntry ||
      entryForm.employee_id ||
      entryForm.amount.trim() ||
      entryForm.quantity.trim() ||
      entryForm.rate.trim() ||
      entryForm.description.trim() !== defaultDesc ||
      notes.trim(),
  );
  useUnsavedChanges(jobRunning || entryDirty);

  useEffect(() => {
    if (!processJob) return;
    if (processJob.status === 'succeeded') {
      const runId = (processJob.result as { run_id?: string } | null)?.run_id;
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollOverview'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard-summary'] });
      setJobId(null);
      if (runId) navigate(`/payroll/${runId}`);
    } else if (processJob.status === 'failed') {
      setProcessError(
        processJob.error
          ? translateServerMessage(processJob.error)
          : t('payroll.process.processFailed'),
      );
      setJobId(null);
    }
  }, [processJob, navigate, queryClient, t]);

  const saveEntryMutation = useMutation({
    mutationFn: () => {
      const amount = Math.round(Number(entryForm.amount) * 100);
      const payload = {
        employee_id: entryForm.employee_id,
        period_year: year,
        period_month: month,
        category: entryForm.category,
        item_type: entryForm.item_type,
        description: entryForm.description,
        amount,
        quantity: entryForm.quantity ? Number(entryForm.quantity) : undefined,
        rate: entryForm.rate ? Math.round(Number(entryForm.rate) * 100) : undefined,
        clear_quantity: editingEntry ? Boolean(editingEntry.quantity && !entryForm.quantity.trim()) : undefined,
        clear_rate: editingEntry ? Boolean(editingEntry.rate && !entryForm.rate.trim()) : undefined,
        is_taxable: entryForm.is_taxable,
      };

      return editingEntry
        ? updatePayrollEntry(editingEntry.id, payload)
        : createPayrollEntry(payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollEntries'] });
      invalidatePreview();
      resetEntryForm();
    },
    onError: (err: unknown) => {
      setEntryError(getErrorMessage(err, t('payroll.process.saveEntryFailed')));
    },
  });

  const deleteEntryMutation = useMutation({
    mutationFn: deletePayrollEntry,
    onSuccess: (_data, deletedId) => {
      queryClient.invalidateQueries({ queryKey: ['payrollEntries'] });
      invalidatePreview();
      if (editingEntry?.id === deletedId) {
        resetEntryForm();
      }
    },
    onError: (err: unknown) => {
      toast.error(getErrorMessage(err, t('payroll.process.deleteEntryFailed')));
    },
  });

  const runRequest = () => ({
    payroll_group_id: groupId,
    period_year: year,
    period_month: month,
    notes: notes || undefined,
  });

  const handleReview = () => {
    if (!groupId) return;
    setProcessError('');
    previewMutation.mutate(runRequest());
  };

  const handleProcess = () => {
    if (!groupId) return;
    mutation.mutate(runRequest());
  };

  /// Any change to the period, the group, or the staged entries makes the
  /// reviewed figures stale, so the preview is discarded rather than left
  /// showing numbers that no longer describe what would be committed.
  const invalidatePreview = () => {
    if (previewMutation.data || previewMutation.isError) previewMutation.reset();
  };

  const resetEntryForm = () => {
    setEditingEntry(null);
    setEntryError('');
    setEntryForm({
      employee_id: '',
      employee_label: '',
      category: 'earning',
      item_type: 'monthly_allowance',
      description: t('payroll.process.defaultDescription'),
      amount: '',
      quantity: '',
      rate: '',
      is_taxable: true,
    });
  };

  const startEditEntry = (entry: PayrollEntryWithEmployee) => {
    if (adjustmentsLocked) return;
    setEditingEntry(entry);
    setEntryError('');
    setEntryForm({
      employee_id: entry.employee_id,
      employee_label: formatEmployeeLabel(entry.employee_name, entry.employee_number) ?? '',
      category: entry.category,
      item_type: entry.item_type,
      description: entry.description,
      amount: (entry.amount / 100).toFixed(2),
      quantity: entry.quantity ? String(entry.quantity) : '',
      rate: entry.rate ? (entry.rate / 100).toFixed(2) : '',
      is_taxable: entry.is_taxable ?? true,
    });
  };

  const handleSaveEntry = () => {
    if (adjustmentsLocked) return;
    setEntryError('');
    if (!entryForm.employee_id || !entryForm.description.trim() || !entryForm.amount.trim()) {
      setEntryError(t('payroll.process.entryRequired'));
      return;
    }
    const parsedAmount = Number(entryForm.amount);
    if (!Number.isFinite(parsedAmount) || parsedAmount <= 0) {
      setEntryError(t('payroll.process.amountPositive'));
      return;
    }
    if (entryForm.quantity.trim()) {
      const parsedQuantity = Number(entryForm.quantity);
      if (!Number.isFinite(parsedQuantity) || parsedQuantity < 0) {
        setEntryError(t('payroll.process.quantityInvalid'));
        return;
      }
    }
    if (entryForm.rate.trim()) {
      const parsedRate = Number(entryForm.rate);
      if (!Number.isFinite(parsedRate) || parsedRate < 0) {
        setEntryError(t('payroll.process.rateInvalid'));
        return;
      }
    }
    saveEntryMutation.mutate();
  };

  const selectedEntryKind = getEntryKind(entryForm);

  return (
    <div className="max-w-6xl space-y-6">
      <PageHeader
        title={t('payroll.process.title')}
        breadcrumbs={[
          { label: t('nav.payroll'), href: '/payroll' },
          { label: t('payroll.process.title') },
        ]}
      />

      <PayrollLifecycleStepper status={preview ? 'processed' : 'draft'} />

      {jobId !== null && jobPollError && (
        <div
          className="rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          role="alert"
        >
          <span>{getErrorMessage(jobPollErrorObj, t('payroll.process.jobPollFailed'))}</span>
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={() => void refetchJob()}
              className="btn-secondary min-h-10"
            >
              <RefreshCw className="w-4 h-4" aria-hidden="true" />
              {t('common.retry')}
            </button>
            <button
              type="button"
              onClick={() => setJobId(null)}
              className="btn-secondary min-h-10"
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      )}

      {preview && (
        <PayrollPreviewPanel
          preview={preview}
          onProcess={handleProcess}
          onBack={() => previewMutation.reset()}
          isProcessing={mutation.isPending || jobRunning}
          processingNote={
            jobRunning
              ? processJob?.progress_total
                ? t('payroll.process.progressLine', {
                    done: processJob.progress_done,
                    total: processJob.progress_total,
                  })
                : t('payroll.process.queuedNote')
              : undefined
          }
        />
      )}

      {processError && (
        <div
          className="whitespace-pre-line rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300"
          role="alert"
        >
          {processError}
        </div>
      )}

      <div className={`card space-y-6 max-w-2xl ${preview ? 'hidden' : ''}`}>
        <FormField label={t('employees.form.payrollGroup')} required>
          {(aria) => (
            <select
              {...aria}
              value={groupId}
              onChange={(e) => {
                setGroupId(e.target.value);
                invalidatePreview();
              }}
              className="form-input"
            >
              <option value="">{t('payroll.process.selectGroup')}</option>
              {groups?.map((g) => (
                <option key={g.id} value={g.id}>
                  {t('payroll.process.groupOption', { name: g.name, cutoff: g.cutoff_day, pay: g.payment_day })}
                </option>
              ))}
            </select>
          )}
        </FormField>
        {groupsError && (
          <div
            role="alert"
            className="rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300 flex items-center justify-between gap-2"
          >
            <span>{getErrorMessage(groupsErrorObj, t('common.error'))}</span>
            <button type="button" onClick={() => void refetchGroups()} className="underline font-medium shrink-0">
              {t('common.retry')}
            </button>
          </div>
        )}

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <FormField label={t('payroll.process.year')} required>
            {(aria) => (
              <select
                {...aria}
                value={year}
                onChange={(e) => {
                  setYear(parseInt(e.target.value, 10));
                  invalidatePreview();
                  if (editingEntry) resetEntryForm();
                }}
                className="form-input"
              >
                {[year - 1, year, year + 1].map((y) => (
                  <option key={y} value={y}>{y}</option>
                ))}
              </select>
            )}
          </FormField>
          <FormField label={t('payroll.process.month')} required>
            {(aria) => (
              <select
                {...aria}
                value={month}
                onChange={(e) => {
                  setMonth(parseInt(e.target.value, 10));
                  invalidatePreview();
                  if (editingEntry) resetEntryForm();
                }}
                className="form-input"
              >
                {MONTH_VALUES.map((m) => (
                  <option key={m} value={m}>{monthName(m)}</option>
                ))}
              </select>
            )}
          </FormField>
        </div>

        <FormField label={t('payroll.process.notesLabel')}>
          {(aria) => (
            <textarea
              {...aria}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={3}
              className="form-input"
              placeholder={t('payroll.process.notesPlaceholder')}
            />
          )}
        </FormField>

        <div className="rounded-xl border border-amber-200 dark:border-amber-900/50 bg-amber-50 dark:bg-amber-950/30 p-4 text-sm text-amber-800 dark:text-amber-200">
          {t('payroll.process.previewNotice')}
        </div>

        <button
          type="button"
          onClick={handleReview}
          disabled={!groupId || previewMutation.isPending}
          className="btn-primary min-h-11 w-full sm:w-auto justify-center px-6"
        >
          <Calculator className="w-4 h-4" aria-hidden="true" />
          {previewMutation.isPending ? t('payroll.process.calculating') : t('payroll.process.reviewButton')}
        </button>
      </div>

      <div className="card overflow-hidden !p-0">
        <div className="flex flex-col gap-2 border-b border-gray-100 dark:border-slate-800 px-6 py-4 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('payroll.process.adjustmentsTitle')}</h2>
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('payroll.process.adjustmentsBody')}
            </p>
          </div>
          {editingEntry && (
            <button
              type="button"
              onClick={resetEntryForm}
              disabled={adjustmentsLocked}
              className="btn-secondary min-h-10 text-sm"
            >
              <X className="w-4 h-4" aria-hidden="true" /> {t('payroll.process.cancelEdit')}
            </button>
          )}
        </div>

        {adjustmentsLocked && (
          <div className="flex items-center gap-2 border-b border-amber-100 dark:border-amber-900/50 bg-amber-50/80 dark:bg-amber-950/30 px-6 py-3 text-xs text-amber-800 dark:text-amber-200">
            <Lock className="w-4 h-4 shrink-0" aria-hidden="true" />
            <span>
              {jobRunning
                ? t('payroll.process.adjustmentsLockedRunning')
                : t('payroll.process.adjustmentsLockedPreview')}
            </span>
          </div>
        )}

        <fieldset
          disabled={adjustmentsLocked}
          className="grid grid-cols-1 gap-4 border-b border-gray-100 dark:border-slate-800 p-6 lg:grid-cols-6 disabled:opacity-60"
        >
          <div className="lg:col-span-2">
            <span className="form-label">{t('employees.columns.employee')} *</span>
            {/* Keyed on the entry so switching between two edits cannot leave the
                previous employee's name in the box. */}
            <EmployeePicker
              key={editingEntry?.id ?? 'new'}
              value={entryForm.employee_id}
              onChange={(id, label) => setEntryForm((prev) => ({ ...prev, employee_id: id, employee_label: label }))}
              initialLabel={entryForm.employee_label || undefined}
            />
          </div>
          <div>
            <FormField label={t('payroll.process.entryKind')} required>
              {(aria) => (
                <select
                  {...aria}
                  value={selectedEntryKind}
                  onChange={(e) => {
                    const kind = e.target.value as EntryKind;
                    setEntryForm((prev) => ({
                      ...prev,
                      category: kind === 'deduction' ? 'deduction' : 'earning',
                      item_type: kind === 'monthly_allowance'
                        ? 'monthly_allowance'
                        : kind === 'deduction'
                          ? 'manual_deduction'
                          : 'other_earning',
                      description: kind === 'monthly_allowance' && !prev.description
                        ? t('payroll.process.entryKinds.monthly_allowance')
                        : prev.description,
                    }));
                  }}
                  className="form-input"
                >
                  <option value="monthly_allowance">{t('payroll.process.entryKinds.monthly_allowance')}</option>
                  <option value="other_earning">{t('payroll.process.entryKinds.other_earning')}</option>
                  <option value="deduction">{t('payroll.process.entryKinds.deduction')}</option>
                </select>
              )}
            </FormField>
          </div>
          <div>
            <FormField label={t('payroll.process.itemType')} required>
              {(aria) => (
                <input
                  {...aria}
                  value={entryForm.item_type}
                  onChange={(e) => setEntryForm((prev) => ({ ...prev, item_type: e.target.value }))}
                  disabled={selectedEntryKind === 'monthly_allowance' || adjustmentsLocked}
                  className="form-input disabled:bg-gray-50 disabled:text-gray-500 dark:disabled:bg-slate-800/60 dark:disabled:text-slate-400"
                  placeholder="monthly_allowance"
                />
              )}
            </FormField>
          </div>
          <div>
            <FormField label={t('payroll.process.amountRm')} required>
              {(aria) => (
                <input
                  {...aria}
                  type="number"
                  min="0"
                  step="0.01"
                  value={entryForm.amount}
                  onChange={(e) => setEntryForm((prev) => ({ ...prev, amount: e.target.value }))}
                  className="form-input"
                  placeholder="0.00"
                />
              )}
            </FormField>
          </div>
          <div className="flex items-end">
            <button
              type="button"
              onClick={handleSaveEntry}
              disabled={adjustmentsLocked || saveEntryMutation.isPending}
              className="btn-primary min-h-11 w-full justify-center"
            >
              <Plus className="w-4 h-4" aria-hidden="true" />
              {saveEntryMutation.isPending ? t('common.saving') : editingEntry ? t('common.update') : t('common.add')}
            </button>
          </div>
          <div className="lg:col-span-3">
            <FormField label={t('payroll.process.description')} required>
              {(aria) => (
                <input
                  {...aria}
                  value={entryForm.description}
                  onChange={(e) => setEntryForm((prev) => ({ ...prev, description: e.target.value }))}
                  className="form-input"
                  placeholder={t('payroll.process.descriptionPlaceholder')}
                />
              )}
            </FormField>
          </div>
          <div>
            <FormField label={t('payroll.process.quantity')}>
              {(aria) => (
                <input
                  {...aria}
                  type="number"
                  min="0"
                  step="0.01"
                  value={entryForm.quantity}
                  onChange={(e) => setEntryForm((prev) => ({ ...prev, quantity: e.target.value }))}
                  className="form-input"
                />
              )}
            </FormField>
          </div>
          <div>
            <FormField label={t('payroll.process.rateRm')}>
              {(aria) => (
                <input
                  {...aria}
                  type="number"
                  min="0"
                  step="0.01"
                  value={entryForm.rate}
                  onChange={(e) => setEntryForm((prev) => ({ ...prev, rate: e.target.value }))}
                  className="form-input"
                />
              )}
            </FormField>
          </div>
          <label
            htmlFor={taxableCheckboxId}
            className="flex items-center gap-2 pt-1 lg:pt-7 text-sm font-medium text-gray-700 dark:text-slate-300"
          >
            <input
              id={taxableCheckboxId}
              type="checkbox"
              checked={entryForm.is_taxable}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, is_taxable: e.target.checked }))}
              className="h-4 w-4 rounded border-gray-300 dark:border-slate-600 text-[var(--accent-1)] focus:ring-[var(--accent-1)]"
            />
            {t('payroll.process.taxable')}
          </label>
          {entryError && (
            <div
              className="lg:col-span-6 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300"
              role="alert"
            >
              {entryError}
            </div>
          )}
        </fieldset>

        {entriesError && (
          <div
            role="alert"
            className="mx-6 mb-4 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300 flex items-center justify-between gap-2"
          >
            <span>{getErrorMessage(entriesErrorObj, t('common.error'))}</span>
            <button type="button" onClick={() => void refetchEntries()} className="underline font-medium shrink-0">
              {t('common.retry')}
            </button>
          </div>
        )}

        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 dark:bg-slate-950/70 text-left">
              <tr>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('employees.columns.employee')}</th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('payroll.process.colType')}</th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('payroll.process.description')}</th>
                <th className="px-6 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('common.amount')}</th>
                <th className="px-6 py-3 text-center text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('payroll.process.taxable')}</th>
                <th className="px-6 py-3 text-center text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-slate-400">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
              {entries.length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-6 py-10 text-center text-sm text-gray-500 dark:text-slate-400">
                    {t('payroll.process.noAdjustments', { period: formatPeriod(year, month) })}
                  </td>
                </tr>
              ) : entries.map((entry) => {
                const isAutoManaged = Boolean(
                  entry.overtime_application_id ||
                    entry.leave_request_id ||
                    entry.item_type === 'overtime' ||
                    entry.item_type === 'claim_reimbursement',
                );
                return (
                <tr key={entry.id} className="hover:bg-gray-50 dark:hover:bg-slate-800/50">
                  <td className="px-6 py-4 text-sm">
                    <div className="font-medium text-gray-900 dark:text-slate-100">{entry.employee_name}</div>
                    <div className="text-xs text-gray-500 dark:text-slate-400">{entry.employee_number}</div>
                  </td>
                  <td className="px-6 py-4 text-sm">
                    <span className={`badge ${entry.category === 'earning' ? 'badge-approved' : 'badge-rejected'}`}>
                      {getEntryKindLabel(entry, t)}
                    </span>
                    <div className="mt-1 text-xs text-gray-500 dark:text-slate-400">
                      {formatEntryItemType(entry.item_type, t)}
                    </div>
                  </td>
                  <td className="px-6 py-4 text-sm text-gray-700 dark:text-slate-300">{entry.description}</td>
                  <td className="px-6 py-4 text-right text-sm font-semibold tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
                    {formatMYR(entry.amount)}
                  </td>
                  <td className="px-6 py-4 text-center text-sm text-gray-600 dark:text-slate-300">
                    {entry.is_taxable ? t('common.yes') : t('common.no')}
                  </td>
                  <td className="px-6 py-4 text-center">
                    {isAutoManaged ? (
                      <span className="text-xs text-gray-400 dark:text-slate-500">
                        {t('payroll.process.approvalManaged')}
                      </span>
                    ) : (
                      <div className="flex items-center justify-center gap-1">
                        <button
                          type="button"
                          onClick={() => startEditEntry(entry)}
                          disabled={adjustmentsLocked}
                          className="inline-flex min-h-11 min-w-11 items-center justify-center rounded-lg text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 transition-colors"
                          aria-label={t('common.edit')}
                          title={t('common.edit')}
                        >
                          <Pencil className="w-4 h-4" aria-hidden="true" />
                        </button>
                        <button
                          type="button"
                          onClick={async () => {
                            if (adjustmentsLocked) return;
                            const ok = await confirmDialog({
                              description: t('payroll.process.deleteEntryConfirm'),
                              variant: 'danger',
                            });
                            if (ok) deleteEntryMutation.mutate(entry.id);
                          }}
                          disabled={adjustmentsLocked || deleteEntryMutation.isPending}
                          className="inline-flex min-h-11 min-w-11 items-center justify-center rounded-lg text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 disabled:opacity-40 transition-colors"
                          aria-label={t('common.delete')}
                          title={t('common.delete')}
                        >
                          <Trash2 className="w-4 h-4" aria-hidden="true" />
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
