import { useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  Ban,
  CheckCircle,
  Clock,
  Download,
  FileText,
  Lock,
  Pencil,
  RotateCcw,
  Save,
  Send,
  Trash2,
  X,
} from 'lucide-react';
import {
  cancelPayrollRun,
  downloadPaymentFile,
  getJournalPreview,
  getPayrollRun,
  getPayrollRunAuditLogs,
  approvePayroll,
  lockPayroll,
  downloadRunPayslips,
  deletePayrollRun,
  returnPayrollForChanges,
  reversePayrollRun,
  submitPayrollForApproval,
  updatePayrollItemPcb,
} from '@/api/payroll';
import { formatMYR, getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatDate, formatDateTime, formatNumber, formatPeriod } from '@/lib/format';
import { PAYROLL_STATUS_STYLES, payrollStatusLabel } from './statusMeta';
import { useAuth } from '@/context/AuthContext';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { usePermissions } from '@/lib/usePermissions';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { useConfirm } from '@/lib/confirm';
import { usePrompt } from '@/lib/prompt';
import { useToast } from '@/lib/toast';
import { PayrollLifecycleStepper } from './PayrollLifecycleStepper';
import type { JournalPreview } from '@/types';
import type { TFunction } from 'i18next';
import i18n from '@/i18n';
import { PayslipBreakdownDrawer } from './PayslipBreakdownDrawer';
import type { PayrollRun, PayrollSummary } from '@/types';

const canDeletePayrollRun = (run: PayrollRun) =>
  ['draft', 'processed'].includes(run.status) && !run.locked_at;
const canDownloadPayslips = (status: string) => ['approved', 'paid'].includes(status);
// Cancellation follows the four-eyes split the backend enforces: the preparer
// withdraws their own draft/processed run; once submitted, only an approver
// can cancel it.
const canCancelAsPreparer = (status: string) => ['draft', 'processed'].includes(status);
const canCancelAsApprover = (status: string) => ['pending_approval', 'approved'].includes(status);

export function PayrollDetail() {
  const { t } = useTranslation();
  const confirmDialog = useConfirm();
  const promptDialog = usePrompt();
  const toast = useToast();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const { can } = usePermissions();
  const [breakdownEmployeeId, setBreakdownEmployeeId] = useState<string | null>(null);
  const [editingPcbEmployeeId, setEditingPcbEmployeeId] = useState<string | null>(null);
  const [pcbInput, setPcbInput] = useState('');
  const [pcbError, setPcbError] = useState('');
  const [lifecycleError, setLifecycleError] = useState('');
  const [journal, setJournal] = useState<JournalPreview | null>(null);
  const [downloadingPayslips, setDownloadingPayslips] = useState(false);
  const [downloadingPayment, setDownloadingPayment] = useState(false);

  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['payrollRun', id],
    queryFn: () => getPayrollRun(id!),
    enabled: !!id,
  });

  const { data: auditLogs = [] } = useQuery({
    queryKey: ['payrollRunAuditLogs', id],
    queryFn: () => getPayrollRunAuditLogs(id!),
    enabled: !!id,
  });

  const invalidateRunQueries = () => {
    queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
    queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
    queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    queryClient.invalidateQueries({ queryKey: ['payrollOverview'] });
    queryClient.invalidateQueries({ queryKey: ['dashboard-summary'] });
  };

  const submitMutation = useMutation({
    mutationFn: () => submitPayrollForApproval(id!),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.submitSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.submitFailed')));
    },
  });

  const approveMutation = useMutation({
    mutationFn: () => approvePayroll(id!),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.approveSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.approveFailed')));
    },
  });

  const returnMutation = useMutation({
    mutationFn: (reason?: string) => returnPayrollForChanges(id!, reason),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.returnSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.returnFailed')));
    },
  });

  const lockMutation = useMutation({
    mutationFn: () => lockPayroll(id!),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.lockSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.lockFailed')));
    },
  });

  const cancelMutation = useMutation({
    mutationFn: (reason: string) => cancelPayrollRun(id!, reason),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.cancelSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.cancelFailed')));
    },
  });

  const reverseMutation = useMutation({
    mutationFn: (reason: string) => reversePayrollRun(id!, reason),
    onSuccess: () => {
      setLifecycleError('');
      invalidateRunQueries();
      toast.success(t('payroll.detail.reverseSuccess'));
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.reverseFailed')));
    },
  });

  const journalMutation = useMutation({
    mutationFn: () => getJournalPreview(id!),
    onSuccess: (preview) => {
      setLifecycleError('');
      setJournal(preview);
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.journalFailed')));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => deletePayrollRun(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollOverview'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard-summary'] });
      toast.success(t('payroll.deleteSuccess'));
      navigate('/payroll');
    },
    onError: (err: unknown) => {
      setLifecycleError(getErrorMessage(err, t('payroll.detail.deleteFailed')));
    },
  });

  const updatePcbMutation = useMutation({
    mutationFn: ({ employeeId, pcbAmount }: { employeeId: string; pcbAmount: number }) =>
      updatePayrollItemPcb(id!, employeeId, { pcb_amount: pcbAmount }),
    onSuccess: (summary, variables) => {
      queryClient.setQueryData(['payrollRun', id], summary);
      queryClient.invalidateQueries({ queryKey: ['payslipBreakdown', id, variables.employeeId] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollOverview'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard-summary'] });
      setEditingPcbEmployeeId(null);
      setPcbInput('');
      setPcbError('');
      toast.success(t('payroll.detail.pcbUpdated'));
    },
    onError: (err: unknown) => {
      setPcbError(getErrorMessage(err, t('payroll.detail.pcbFailed')));
    },
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64" role="status">
        <span className="sr-only">{t('common.loading')}</span>
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--accent-1)]" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="card">
        <EmptyState
          icon={AlertTriangle}
          title={t('payroll.detail.loadFailed')}
          description={getErrorMessage(error, t('payroll.detail.loadFailed'))}
          action={
            <button type="button" onClick={() => void refetch()} className="btn-primary">
              {t('common.retry')}
            </button>
          }
        />
      </div>
    );
  }

  if (!data) {
    return <div className="text-center text-gray-500 dark:text-slate-400 py-12">{t('payroll.detail.notFound')}</div>;
  }

  const { payroll_run: run, items } = data;
  const canManageDraft = can('manage_payroll_draft');
  const canSubmit = can('submit_payroll');
  const canApprove = can('approve_payroll');
  const canPay = can('mark_payroll_paid');
  const isSuperAdmin = hasAnyRole(user, SUPER_ADMIN_ROLES);
  const isOwnRun =
    Boolean(user?.id) &&
    (run.processed_by === user?.id || run.submitted_by === user?.id || run.updated_by === user?.id);
  const selfApprovalBlocked = isOwnRun && !isSuperAdmin;
  const selfApprovalOverride = isOwnRun && isSuperAdmin;
  const showCancel =
    (canManageDraft && canCancelAsPreparer(run.status)) ||
    (canApprove && canCancelAsApprover(run.status));
  const canEditPcb = canManageDraft && run.status === 'processed';
  const periodLabel = formatPeriod(run.period_year, run.period_month);

  const handleDownloadPayslips = async () => {
    setDownloadingPayslips(true);
    try {
      await downloadRunPayslips(id!);
    } catch (err) {
      toast.error(getErrorMessage(err, t('payroll.detail.downloadFailed')));
    } finally {
      setDownloadingPayslips(false);
    }
  };

  const handleDownloadPaymentFile = async () => {
    setDownloadingPayment(true);
    try {
      await downloadPaymentFile(id!);
    } catch (err) {
      toast.error(getErrorMessage(err, t('payroll.detail.downloadFailed')));
    } finally {
      setDownloadingPayment(false);
    }
  };

  const startEditPcb = (employeeId: string, pcbAmount: number) => {
    setEditingPcbEmployeeId(employeeId);
    setPcbInput((pcbAmount / 100).toFixed(2));
    setPcbError('');
  };

  const cancelEditPcb = () => {
    setEditingPcbEmployeeId(null);
    setPcbInput('');
    setPcbError('');
  };

  const savePcb = (employeeId: string) => {
    if (!pcbInput.trim()) {
      setPcbError(t('payroll.detail.pcbInvalid'));
      return;
    }
    const amount = Number(pcbInput);
    if (!Number.isFinite(amount) || amount < 0) {
      setPcbError(t('payroll.detail.pcbInvalid'));
      return;
    }
    updatePcbMutation.mutate({ employeeId, pcbAmount: Math.round(amount * 100) });
  };

  return (
    <div>
      <PageHeader
        title={t('payroll.detail.title', { period: periodLabel })}
        subtitle={t('payroll.detail.employeesProcessed', { count: run.employee_count })}
        breadcrumbs={[
          { label: t('nav.payroll'), href: '/payroll' },
          { label: t('payroll.listTitle'), href: '/payroll/runs' },
          { label: periodLabel },
        ]}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            {canDownloadPayslips(run.status) && (
              <button
                type="button"
                onClick={handleDownloadPayslips}
                disabled={downloadingPayslips}
                className="no-print btn-secondary min-h-11 disabled:opacity-50"
              >
                <Download className="w-4 h-4" aria-hidden="true" />
                {downloadingPayslips ? t('common.downloading') : t('payroll.detail.downloadPayslips')}
              </button>
            )}
            {canManageDraft && canDeletePayrollRun(run) && (
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    description: t('payroll.deleteConfirm', { period: periodLabel }),
                    variant: 'danger',
                  });
                  if (ok) {
                    deleteMutation.mutate();
                  }
                }}
                disabled={deleteMutation.isPending}
                className="no-print flex items-center gap-2 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400 px-4 py-2 min-h-[44px] rounded-xl hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 text-sm font-medium transition-colors"
              >
                <Trash2 className="w-4 h-4" aria-hidden="true" />
                {deleteMutation.isPending ? t('common.deleting') : t('common.delete')}
              </button>
            )}
            {canSubmit && run.status === 'processed' && (
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    title: t('payroll.detail.submitForApproval'),
                    description: t('payroll.detail.submitConfirm', { period: periodLabel }),
                    confirmLabel: t('payroll.detail.submitForApproval'),
                    variant: 'primary',
                  });
                  if (ok) submitMutation.mutate();
                }}
                disabled={submitMutation.isPending}
                className="no-print btn-primary min-h-11"
              >
                <Send className="w-4 h-4" aria-hidden="true" />
                {submitMutation.isPending ? t('payroll.detail.submitting') : t('payroll.detail.submitForApproval')}
              </button>
            )}
            {canApprove && run.status === 'pending_approval' && (
              <>
                <button
                  type="button"
                  onClick={async () => {
                    const reason = await promptDialog({
                      title: t('payroll.detail.return'),
                      description: t('payroll.detail.returnPrompt'),
                      confirmLabel: t('payroll.detail.return'),
                      variant: 'warning',
                    });
                    if (reason !== null) returnMutation.mutate(reason || undefined);
                  }}
                  disabled={returnMutation.isPending}
                  className="no-print btn-secondary min-h-11"
                >
                  <RotateCcw className="w-4 h-4" aria-hidden="true" />
                  {returnMutation.isPending ? t('payroll.detail.returning') : t('payroll.detail.return')}
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    if (selfApprovalBlocked) return;
                    const ok = await confirmDialog({
                      title: t('payroll.detail.approve'),
                      description: selfApprovalOverride
                        ? t('payroll.detail.approveOverrideConfirm', { period: periodLabel })
                        : t('payroll.detail.approveConfirm', { period: periodLabel }),
                      confirmLabel: t('payroll.detail.approve'),
                      variant: selfApprovalOverride ? 'warning' : 'primary',
                    });
                    if (ok) approveMutation.mutate();
                  }}
                  disabled={approveMutation.isPending || selfApprovalBlocked}
                  title={selfApprovalBlocked ? t('payroll.detail.selfApprovalBlocked') : undefined}
                  className="no-print flex items-center gap-2 bg-emerald-600 text-white px-4 py-2 min-h-[44px] rounded-xl hover:bg-emerald-700 disabled:opacity-50 text-sm font-semibold transition-colors"
                >
                  <CheckCircle className="w-4 h-4" aria-hidden="true" />
                  {approveMutation.isPending ? t('payroll.detail.approving') : t('payroll.detail.approve')}
                </button>
              </>
            )}
            {canPay && run.status === 'approved' && (
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    title: t('payroll.detail.lockMarkPaid'),
                    description: t('payroll.detail.lockConfirm', { period: periodLabel }),
                    confirmLabel: t('payroll.detail.lockMarkPaid'),
                    variant: 'primary',
                  });
                  if (ok) lockMutation.mutate();
                }}
                disabled={lockMutation.isPending}
                className="no-print btn-primary min-h-11"
              >
                <Lock className="w-4 h-4" aria-hidden="true" />
                {lockMutation.isPending ? t('payroll.detail.locking') : t('payroll.detail.lockMarkPaid')}
              </button>
            )}
            {canPay && canDownloadPayslips(run.status) && (
              <>
                <button
                  type="button"
                  onClick={handleDownloadPaymentFile}
                  disabled={downloadingPayment}
                  className="no-print btn-secondary min-h-11 disabled:opacity-50"
                >
                  <Download className="w-4 h-4" aria-hidden="true" />
                  {downloadingPayment ? t('common.downloading') : t('payroll.detail.paymentFile')}
                </button>
                <button
                  type="button"
                  onClick={() => journalMutation.mutate()}
                  disabled={journalMutation.isPending}
                  className="no-print btn-secondary min-h-11"
                >
                  <FileText className="w-4 h-4" aria-hidden="true" />
                  {journalMutation.isPending ? t('common.loading') : t('payroll.detail.journalPreview')}
                </button>
              </>
            )}
            {showCancel && (
              <button
                type="button"
                onClick={async () => {
                  const reason = await promptDialog({
                    title: t('payroll.detail.cancelRun'),
                    description: t('payroll.detail.cancelPrompt'),
                    confirmLabel: t('payroll.detail.cancelRun'),
                    required: true,
                    variant: 'danger',
                  });
                  if (reason) cancelMutation.mutate(reason);
                }}
                disabled={cancelMutation.isPending}
                className="no-print flex items-center gap-2 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400 px-4 py-2 min-h-[44px] rounded-xl hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 text-sm font-medium transition-colors"
              >
                <Ban className="w-4 h-4" aria-hidden="true" />
                {cancelMutation.isPending ? t('payroll.detail.cancelling') : t('payroll.detail.cancelRun')}
              </button>
            )}
            {canPay && run.status === 'paid' && (
              <button
                type="button"
                onClick={async () => {
                  const reason = await promptDialog({
                    title: t('payroll.detail.reverseRun'),
                    description: t('payroll.detail.reversePrompt'),
                    confirmLabel: t('payroll.detail.reverseRun'),
                    required: true,
                    variant: 'danger',
                  });
                  if (reason) reverseMutation.mutate(reason);
                }}
                disabled={reverseMutation.isPending}
                className="no-print flex items-center gap-2 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400 px-4 py-2 min-h-[44px] rounded-xl hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 text-sm font-medium transition-colors"
              >
                <RotateCcw className="w-4 h-4" aria-hidden="true" />
                {reverseMutation.isPending ? t('payroll.detail.reversing') : t('payroll.detail.reverseRun')}
              </button>
            )}
            <span className={`px-3 py-1 rounded-full text-sm font-medium ${PAYROLL_STATUS_STYLES[run.status] ?? 'bg-gray-100 text-gray-900 dark:bg-slate-800 dark:text-slate-200'}`}>
              {payrollStatusLabel(run.status)}
            </span>
          </div>
        }
      />

      <PayrollLifecycleStepper status={run.status} />
      {lifecycleError && (
        <div
          className="mb-6 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300"
          role="alert"
        >
          {lifecycleError}
        </div>
      )}
      {canApprove && run.status === 'pending_approval' && selfApprovalBlocked && (
        <div
          className="no-print mb-6 rounded-xl border border-amber-200 dark:border-amber-900/50 bg-amber-50 dark:bg-amber-950/30 px-4 py-3 text-sm text-amber-800 dark:text-amber-200"
          role="alert"
        >
          {t('payroll.detail.selfApprovalBlocked')}
        </div>
      )}
      {canApprove && run.status === 'pending_approval' && selfApprovalOverride && (
        <div
          className="no-print mb-6 rounded-xl border border-amber-200 dark:border-amber-900/50 bg-amber-50 dark:bg-amber-950/30 px-4 py-3 text-sm text-amber-800 dark:text-amber-200"
          role="status"
        >
          {t('payroll.detail.selfApprovalOverrideNotice')}
        </div>
      )}
      {run.status === 'paid' && (
        <div className="mb-6 rounded-xl border border-emerald-200 dark:border-emerald-900/50 bg-emerald-50 dark:bg-emerald-950/30 px-4 py-3 text-sm text-emerald-700 dark:text-emerald-300">
          {t('payroll.detail.paidNotice')}
        </div>
      )}
      {run.status === 'pending_approval' && (
        <div className="mb-6 rounded-xl border border-blue-200 dark:border-blue-900/50 bg-blue-50 dark:bg-blue-950/30 px-4 py-3 text-sm text-blue-700 dark:text-blue-300">
          {t('payroll.detail.pendingNotice')}
        </div>
      )}
      {run.status === 'cancelled' && (
        <div className="mb-6 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300">
          {t('payroll.detail.cancelledNotice', {
            reason: run.cancel_reason ? t('payroll.detail.cancelledReason', { reason: run.cancel_reason }) : '',
            date: run.cancelled_at ? t('payroll.detail.cancelledOn', { date: formatDate(run.cancelled_at) }) : '',
          })}
        </div>
      )}

      <PayrollAuditTimeline logs={auditLogs} />

      {/* Summary Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.totals.gross')}</p>
          <p className="text-lg font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">{formatMYR(run.total_gross)}</p>
        </div>
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.totals.net')}</p>
          <p className="text-lg font-bold mt-1 tabular-nums text-emerald-600 dark:text-emerald-400">{formatMYR(run.total_net)}</p>
        </div>
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.totals.employerCost')}</p>
          <p className="text-lg font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">{formatMYR(run.total_employer_cost)}</p>
        </div>
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.totals.pcb')}</p>
          <p className="text-lg font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">{formatMYR(run.total_pcb)}</p>
        </div>
      </div>

      {/* Statutory Summary */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-6">
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.preview.epfFull')}</p>
          <p className="text-sm font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">
            {formatMYR(run.total_epf_employee)} + {formatMYR(run.total_epf_employer)}
          </p>
        </div>
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.preview.socsoFull')}</p>
          <p className="text-sm font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">
            {formatMYR(run.total_socso_employee)} + {formatMYR(run.total_socso_employer)}
          </p>
        </div>
        <div className="card !p-4">
          <p className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.preview.eisFull')}</p>
          <p className="text-sm font-bold mt-1 tabular-nums text-gray-900 dark:text-slate-100">
            {formatMYR(run.total_eis_employee)} + {formatMYR(run.total_eis_employer)}
          </p>
        </div>
      </div>

      {/* Employee Details Table */}
      <div className="card overflow-hidden !p-0">
        <div className="px-6 py-4 border-b border-gray-200 dark:border-slate-800">
          <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('payroll.detail.breakdownTitle')}</h2>
          <p className="mt-1 text-sm text-gray-500 dark:text-slate-400">
            {t('payroll.detail.breakdownBody')}
            {canEditPcb && ` ${t('payroll.detail.breakdownPcbHint')}`}
          </p>
          {pcbError && (
            <div
              className="mt-3 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300"
              role="alert"
            >
              {pcbError}
            </div>
          )}
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 dark:bg-slate-950/70 border-b border-gray-200 dark:border-slate-800">
              <tr>
                <th className="sticky left-0 z-10 bg-gray-50 dark:bg-slate-950 text-left px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">
                  {t('employees.columns.employee')}
                </th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.basic')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.allowances')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.ot')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.gross')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">EPF</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">SOCSO</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">EIS</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">PCB</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.deductions')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.claims')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('payroll.cols.net')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
              {items.map((item) => (
                <tr key={item.employee_id} className="hover:bg-gray-50 dark:hover:bg-slate-800/50">
                  <td className="sticky left-0 z-10 bg-white dark:bg-slate-900 px-4 py-3 whitespace-nowrap">
                    <button
                      type="button"
                      onClick={() => setBreakdownEmployeeId(item.employee_id)}
                      className="text-left min-h-11 py-1 rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-1)]"
                    >
                      <div className="text-sm font-medium text-gray-900 dark:text-slate-100 underline decoration-gray-300 dark:decoration-slate-600 underline-offset-2 hover:decoration-gray-900 dark:hover:decoration-slate-200">
                        {item.employee_name}
                      </div>
                      <div className="text-xs text-gray-500 dark:text-slate-400">{item.employee_number}</div>
                    </button>
                  </td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{formatMYR(item.basic_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{item.total_allowances > 0 ? formatMYR(item.total_allowances) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{item.total_overtime > 0 ? formatMYR(item.total_overtime) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{formatMYR(item.gross_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{formatMYR(item.epf_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{formatMYR(item.socso_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">{formatMYR(item.eis_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
                    {editingPcbEmployeeId === item.employee_id ? (
                      <div className="no-print flex min-w-44 items-center justify-end gap-1.5">
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          value={pcbInput}
                          onChange={(e) => setPcbInput(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              savePcb(item.employee_id);
                            } else if (e.key === 'Escape') {
                              e.preventDefault();
                              cancelEditPcb();
                            }
                          }}
                          aria-label={t('payroll.detail.pcbInputLabel', { name: item.employee_name })}
                          className="w-24 rounded-lg border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 px-2 py-1.5 min-h-11 sm:min-h-9 text-right text-sm outline-none focus:border-[var(--accent-1)] focus:ring-1 focus:ring-[var(--accent-1)]"
                        />
                        <button
                          type="button"
                          onClick={() => savePcb(item.employee_id)}
                          disabled={updatePcbMutation.isPending}
                          className="inline-flex min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 items-center justify-center rounded-lg text-emerald-600 dark:text-emerald-400 hover:bg-emerald-50 dark:hover:bg-emerald-950/30 disabled:opacity-50"
                          aria-label={t('payroll.detail.savePcb')}
                          title={t('payroll.detail.savePcb')}
                        >
                          <Save className="w-4 h-4" aria-hidden="true" />
                        </button>
                        <button
                          type="button"
                          onClick={cancelEditPcb}
                          className="inline-flex min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800"
                          aria-label={t('payroll.detail.cancelPcbEdit')}
                          title={t('payroll.detail.cancelPcbEdit')}
                        >
                          <X className="w-4 h-4" aria-hidden="true" />
                        </button>
                      </div>
                    ) : (
                      <div className="flex items-center justify-end gap-1.5">
                        <span>{formatMYR(item.pcb_amount)}</span>
                        {canEditPcb && (
                          <button
                            type="button"
                            onClick={() => startEditPcb(item.employee_id, item.pcb_amount)}
                            className="no-print inline-flex min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 hover:text-gray-900 dark:hover:text-slate-100"
                            aria-label={t('payroll.detail.editPcb')}
                            title={t('payroll.detail.editPcb')}
                          >
                            <Pencil className="w-4 h-4" aria-hidden="true" />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap text-red-600 dark:text-red-400">{formatMYR(item.total_deductions)}</td>
                  <td className="px-4 py-3 text-sm text-right tabular-nums whitespace-nowrap">
                    {item.total_claims > 0 ? <span className="text-blue-600 dark:text-blue-400">{formatMYR(item.total_claims)}</span> : '-'}
                  </td>
                  <td className="px-4 py-3 text-sm text-right font-bold tabular-nums whitespace-nowrap text-emerald-700 dark:text-emerald-400">{formatMYR(item.net_salary)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <CalculationProvenance snapshot={data.calculation_snapshot} />

      {breakdownEmployeeId && (
        <PayslipBreakdownDrawer
          runId={id!}
          employeeId={breakdownEmployeeId}
          onClose={() => setBreakdownEmployeeId(null)}
        />
      )}

      <Modal
        open={journal !== null}
        onClose={() => setJournal(null)}
        title={t('payroll.detail.journalTitle', {
          period: journal ? formatPeriod(journal.period_year, journal.period_month) : '',
        })}
        maxWidth="max-w-3xl"
      >
        {journal && (
          <div>
            {!journal.balanced && (
              <div
                className="mb-4 rounded-xl border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 text-sm text-red-700 dark:text-red-300"
                role="alert"
              >
                {t('payroll.detail.journalUnbalanced', {
                  debits: formatMYR(journal.total_debits),
                  credits: formatMYR(journal.total_credits),
                })}
              </div>
            )}
            <div className="overflow-x-auto">
              <table className="w-full text-sm text-gray-900 dark:text-slate-100">
                <thead>
                  <tr className="text-left text-xs text-gray-500 dark:text-slate-400 border-b border-gray-100 dark:border-slate-800">
                    <th className="py-2 pr-4 font-medium whitespace-nowrap">{t('payroll.detail.journalCols.account')}</th>
                    <th className="py-2 pr-4 font-medium whitespace-nowrap">{t('payroll.detail.journalCols.department')}</th>
                    <th className="py-2 font-medium text-right whitespace-nowrap">{t('payroll.detail.journalCols.debit')}</th>
                    <th className="py-2 pl-4 font-medium text-right whitespace-nowrap">{t('payroll.detail.journalCols.credit')}</th>
                  </tr>
                </thead>
                <tbody>
                  {journal.lines.map((line, i) => (
                    <tr key={i} className="border-b border-gray-50 dark:border-slate-800/60 last:border-0">
                      <td className="py-2 pr-4">
                        <span className="font-medium">{line.account_code}</span>{' '}
                        <span className="text-gray-600 dark:text-slate-300">{line.account_name}</span>
                        {line.memo && <div className="text-xs text-gray-500 dark:text-slate-400">{line.memo}</div>}
                      </td>
                      <td className="py-2 pr-4 text-gray-600 dark:text-slate-400">{line.department ?? '—'}</td>
                      <td className="py-2 text-right tabular-nums whitespace-nowrap">
                        {line.side === 'debit' ? formatMYR(line.amount) : ''}
                      </td>
                      <td className="py-2 pl-4 text-right tabular-nums whitespace-nowrap">
                        {line.side === 'credit' ? formatMYR(line.amount) : ''}
                      </td>
                    </tr>
                  ))}
                  <tr className="border-t border-gray-200 dark:border-slate-700 font-semibold">
                    <td className="py-2 pr-4" colSpan={2}>{t('payroll.detail.journalTotals')}</td>
                    <td className="py-2 text-right tabular-nums whitespace-nowrap">{formatMYR(journal.total_debits)}</td>
                    <td className="py-2 pl-4 text-right tabular-nums whitespace-nowrap">{formatMYR(journal.total_credits)}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <ul className="mt-4 list-disc pl-5 text-xs text-gray-500 dark:text-slate-400 space-y-1">
              {journal.notes.map((note, i) => (
                <li key={i}>{translateServerMessage(note)}</li>
              ))}
            </ul>
          </div>
        )}
      </Modal>
    </div>
  );
}

/// What produced this run's figures.
///
/// The statutory rule tables and the company's overtime multipliers are both
/// mutable and effective-dated, so a run recomputed today may not match what was
/// paid. The run records its inputs at commit time; this surfaces them.
function CalculationProvenance({ snapshot }: { snapshot: PayrollSummary['calculation_snapshot'] }) {
  const { t } = useTranslation();
  if (!snapshot) return null;

  const ot = snapshot.overtime_settings;

  return (
    <div className="card !p-4 mt-6">
      <h2 className="mb-1 text-sm font-semibold text-gray-900 dark:text-slate-100">{t('payroll.detail.basisTitle')}</h2>
      <p className="mb-3 text-xs text-gray-500 dark:text-slate-400">
        {t('payroll.detail.basisBody', { date: formatDate(snapshot.effective_date) })}
      </p>
      <div className="overflow-x-auto">
        <table className="w-full text-sm text-gray-900 dark:text-slate-100">
          <thead className="text-left text-xs uppercase tracking-wide text-gray-500 dark:text-slate-400">
            <tr>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.domain')}</th>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.dataset')}</th>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.sourceVersion')}</th>
              <th className="py-1">{t('payroll.detail.basisCols.effective')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
            {snapshot.statutory_rule_sets.map((ruleSet) => (
              <tr key={ruleSet.rule_set_id}>
                <td className="py-1.5 pr-4 font-medium uppercase whitespace-nowrap">{ruleSet.rule_code}</td>
                <td className="py-1.5 pr-4 text-gray-600 dark:text-slate-300">{ruleSet.dataset_key}</td>
                <td className="py-1.5 pr-4 text-gray-600 dark:text-slate-300">{ruleSet.source_version ?? '—'}</td>
                <td className="py-1.5 text-gray-600 dark:text-slate-300 whitespace-nowrap">
                  {formatDate(ruleSet.effective_from)} → {ruleSet.effective_to ? formatDate(ruleSet.effective_to) : t('payroll.detail.effectiveOpen')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-3 text-xs text-gray-500 dark:text-slate-400">
        {t('payroll.detail.otRated', {
          normal: ot.multiplier_normal,
          restDay: ot.multiplier_rest_day,
          publicHoliday: ot.multiplier_public_holiday,
          daysPerMonth: ot.working_days_per_month,
          hoursPerDay: ot.effective_hours_per_day,
        })}
      </p>
    </div>
  );
}

function PayrollAuditTimeline({ logs }: { logs: Awaited<ReturnType<typeof getPayrollRunAuditLogs>> }) {
  const { t } = useTranslation();
  const [showAllLogs, setShowAllLogs] = useState(false);
  const visibleLogs = showAllLogs ? logs : logs.slice(0, 6);

  return (
    <div className="card !p-4 mb-6">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('payroll.detail.auditTitle')}</h2>
        <div className="flex items-center gap-3">
          <span className="text-xs text-gray-500 dark:text-slate-400">{t('payroll.detail.auditRecords', { count: logs.length })}</span>
          {logs.length > 6 && (
            <button
              type="button"
              onClick={() => setShowAllLogs((prev) => !prev)}
              className="no-print text-xs font-medium text-[var(--accent-1)] hover:underline min-h-9 px-1"
            >
              {showAllLogs ? t('payroll.detail.showFewerLogs') : t('payroll.detail.showAllLogs', { count: formatNumber(logs.length) })}
            </button>
          )}
        </div>
      </div>
      {visibleLogs.length === 0 ? (
        <p className="text-sm text-gray-500 dark:text-slate-400">{t('payroll.detail.auditEmpty')}</p>
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {visibleLogs.map((log) => (
            <div key={log.id} className="rounded-xl border border-gray-100 dark:border-slate-800 px-3 py-2.5 bg-gray-50/40 dark:bg-slate-950/40">
              <div className="mb-1 flex items-center justify-between gap-2">
                <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${getAuditActionClass(log.action)}`}>
                  {formatAuditAction(log.action, t)}
                </span>
                <span className="inline-flex items-center gap-1 text-xs text-gray-500 dark:text-slate-400">
                  <Clock className="h-3 w-3" aria-hidden="true" />
                  {formatDateTime(log.created_at)}
                </span>
              </div>
              <p className="truncate text-sm text-gray-700 dark:text-slate-200">{log.description || t('payroll.detail.auditEventFallback')}</p>
              <p className="mt-0.5 truncate text-xs text-gray-500 dark:text-slate-400">{log.user_full_name || log.user_email || t('payroll.detail.system')}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function getAuditActionClass(action: string) {
  const classes: Record<string, string> = {
    process: 'bg-indigo-50 text-indigo-700 dark:bg-indigo-950/50 dark:text-indigo-300',
    approve: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-300',
    self_approval_override: 'bg-amber-50 text-amber-800 dark:bg-amber-950/50 dark:text-amber-300',
    submit_approval: 'bg-blue-50 text-blue-700 dark:bg-blue-950/50 dark:text-blue-300',
    return: 'bg-amber-50 text-amber-700 dark:bg-amber-950/50 dark:text-amber-300',
    return_changes: 'bg-amber-50 text-amber-700 dark:bg-amber-950/50 dark:text-amber-300',
    lock: 'bg-gray-900 text-white dark:bg-slate-700 dark:text-slate-100',
    update: 'bg-blue-50 text-blue-700 dark:bg-blue-950/50 dark:text-blue-300',
    cancel: 'bg-red-50 text-red-700 dark:bg-red-950/50 dark:text-red-300',
    reverse: 'bg-red-50 text-red-700 dark:bg-red-950/50 dark:text-red-300',
    delete: 'bg-red-50 text-red-700 dark:bg-red-950/50 dark:text-red-300',
  };
  return classes[action] ?? 'bg-gray-100 text-gray-700 dark:bg-slate-800 dark:text-slate-300';
}

function formatAuditAction(action: string, t: TFunction) {
  const key = `payroll.detail.auditActions.${action}`;
  return i18n.exists(key) ? t(key) : action.replace(/_/g, ' ').replace(/^\w/, (char) => char.toUpperCase());
}
