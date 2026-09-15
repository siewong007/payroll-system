import { useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Ban, CheckCircle, Clock, Download, FileText, Lock, Pencil, RotateCcw, Save, Send, Trash2, X } from 'lucide-react';
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
  submitPayrollForApproval,
  updatePayrollItemPcb,
} from '@/api/payroll';
import { formatMYR, getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatDate, formatDateTime, formatPeriod } from '@/lib/format';
import { payrollStatusLabel } from './statusMeta';
import { useAuth } from '@/context/AuthContext';
import { canApprovePayroll, canPreparePayroll } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
import { Modal } from '@/components/ui/Modal';
import type { JournalPreview } from '@/types';
import type { TFunction } from 'i18next';
import i18n from '@/i18n';
import { PayslipBreakdownDrawer } from './PayslipBreakdownDrawer';
import type { PayrollSummary } from '@/types';

const canDeletePayrollRun = (status: string) => ['draft', 'processed', 'cancelled'].includes(status);
const canDownloadPayslips = (status: string) => ['approved', 'paid'].includes(status);
// Cancellation follows the four-eyes split the backend enforces: the preparer
// withdraws their own draft/processed run; once submitted, only an approver
// can cancel it.
const canCancelAsPreparer = (status: string) => ['draft', 'processed'].includes(status);
const canCancelAsApprover = (status: string) => ['pending_approval', 'approved'].includes(status);

export function PayrollDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const [breakdownEmployeeId, setBreakdownEmployeeId] = useState<string | null>(null);
  const [editingPcbEmployeeId, setEditingPcbEmployeeId] = useState<string | null>(null);
  const [pcbInput, setPcbInput] = useState('');
  const [pcbError, setPcbError] = useState('');
  const [lifecycleError, setLifecycleError] = useState('');
  const [journal, setJournal] = useState<JournalPreview | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ['payrollRun', id],
    queryFn: () => getPayrollRun(id!),
    enabled: !!id,
  });

  const { data: auditLogs = [] } = useQuery({
    queryKey: ['payrollRunAuditLogs', id],
    queryFn: () => getPayrollRunAuditLogs(id!),
    enabled: !!id,
  });

  const submitMutation = useMutation({
    mutationFn: () => submitPayrollForApproval(id!),
    onSuccess: () => {
      setLifecycleError('');
      queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.submitFailed')));
    },
  });

  const approveMutation = useMutation({
    mutationFn: () => approvePayroll(id!),
    onSuccess: () => {
      setLifecycleError('');
      queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.approveFailed')));
    },
  });

  const returnMutation = useMutation({
    mutationFn: (reason?: string) => returnPayrollForChanges(id!, reason),
    onSuccess: () => {
      setLifecycleError('');
      queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.returnFailed')));
    },
  });

  const lockMutation = useMutation({
    mutationFn: () => lockPayroll(id!),
    onSuccess: () => {
      setLifecycleError('');
      queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.lockFailed')));
    },
  });

  const cancelMutation = useMutation({
    mutationFn: (reason: string) => cancelPayrollRun(id!, reason),
    onSuccess: () => {
      setLifecycleError('');
      queryClient.invalidateQueries({ queryKey: ['payrollRun', id] });
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.cancelFailed')));
    },
  });

  const journalMutation = useMutation({
    mutationFn: () => getJournalPreview(id!),
    onSuccess: (preview) => {
      setLifecycleError('');
      setJournal(preview);
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.journalFailed')));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => deletePayrollRun(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      navigate('/payroll');
    },
    onError: (error: unknown) => {
      setLifecycleError(getErrorMessage(error, t('payroll.detail.deleteFailed')));
    },
  });

  const updatePcbMutation = useMutation({
    mutationFn: ({ employeeId, pcbAmount }: { employeeId: string; pcbAmount: number }) =>
      updatePayrollItemPcb(id!, employeeId, { pcb_amount: pcbAmount }),
    onSuccess: (summary) => {
      queryClient.setQueryData(['payrollRun', id], summary);
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollRunAuditLogs', id] });
      setEditingPcbEmployeeId(null);
      setPcbInput('');
      setPcbError('');
    },
    onError: (err: unknown) => {
      setPcbError(getErrorMessage(err, t('payroll.detail.pcbFailed')));
    },
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  if (!data) return <div className="text-center text-gray-500 py-12">{t('payroll.detail.notFound')}</div>;

  const { payroll_run: run, items } = data;
  const canPrepare = canPreparePayroll(user);
  const canApprove = canApprovePayroll(user);
  const canPay = userCan(user, 'mark_payroll_paid');
  const showCancel =
    (canPrepare && canCancelAsPreparer(run.status)) ||
    (canApprove && canCancelAsApprover(run.status));
  const canEditPcb = canPrepare && run.status === 'processed';

  const startEditPcb = (employeeId: string, pcbAmount: number) => {
    setEditingPcbEmployeeId(employeeId);
    setPcbInput((pcbAmount / 100).toFixed(2));
    setPcbError('');
  };

  const savePcb = (employeeId: string) => {
    const amount = Number(pcbInput);
    if (!Number.isFinite(amount) || amount < 0) {
      setPcbError(t('payroll.detail.pcbInvalid'));
      return;
    }
    updatePcbMutation.mutate({ employeeId, pcbAmount: Math.round(amount * 100) });
  };

  return (
    <div>
      <button
        onClick={() => navigate('/payroll')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> {t('payroll.process.backToPayroll')}
      </button>

      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">
            {t('payroll.detail.title', { period: formatPeriod(run.period_year, run.period_month) })}
          </h1>
          <p className="text-gray-500">{t('payroll.detail.employeesProcessed', { count: run.employee_count })}</p>
        </div>
        <div className="flex items-center gap-3">
          {canDownloadPayslips(run.status) && (
            <button
              onClick={() => downloadRunPayslips(id!)}
              className="flex items-center gap-2 bg-white border border-gray-200 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 text-sm font-medium"
            >
              <Download className="w-4 h-4" /> {t('payroll.detail.downloadPayslips')}
            </button>
          )}
          {canPrepare && canDeletePayrollRun(run.status) && (
            <button
              onClick={() => {
                if (confirm(t('payroll.deleteConfirm', { period: formatPeriod(run.period_year, run.period_month) }))) {
                  deleteMutation.mutate();
                }
              }}
              disabled={deleteMutation.isPending}
              className="flex items-center gap-2 bg-white border border-red-200 text-red-600 px-4 py-2 rounded-lg hover:bg-red-50 disabled:opacity-50 text-sm font-medium"
            >
              <Trash2 className="w-4 h-4" />
              {deleteMutation.isPending ? t('common.deleting') : t('common.delete')}
            </button>
          )}
          {canPrepare && run.status === 'processed' && (
            <button
              onClick={() => submitMutation.mutate()}
              disabled={submitMutation.isPending}
              className="flex items-center gap-2 bg-blue-600 text-white px-4 py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 text-sm font-medium"
            >
              <Send className="w-4 h-4" />
              {submitMutation.isPending ? t('payroll.detail.submitting') : t('payroll.detail.submitForApproval')}
            </button>
          )}
          {canApprove && run.status === 'pending_approval' && (
            <>
              <button
                onClick={() => {
                  const reason = prompt(t('payroll.detail.returnPrompt'))?.trim();
                  if (reason !== undefined) returnMutation.mutate(reason || undefined);
                }}
                disabled={returnMutation.isPending}
                className="flex items-center gap-2 bg-white border border-gray-200 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 disabled:opacity-50 text-sm font-medium"
              >
                <RotateCcw className="w-4 h-4" />
                {returnMutation.isPending ? t('payroll.detail.returning') : t('payroll.detail.return')}
              </button>
              <button
                onClick={() => approveMutation.mutate()}
                disabled={approveMutation.isPending}
                className="flex items-center gap-2 bg-green-600 text-white px-4 py-2 rounded-lg hover:bg-green-700 disabled:opacity-50 text-sm font-medium"
              >
                <CheckCircle className="w-4 h-4" />
                {approveMutation.isPending ? t('payroll.detail.approving') : t('payroll.detail.approve')}
              </button>
            </>
          )}
          {canApprove && run.status === 'approved' && (
            <button
              onClick={() => lockMutation.mutate()}
              disabled={lockMutation.isPending}
              className="flex items-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 disabled:opacity-50 text-sm font-medium"
            >
              <Lock className="w-4 h-4" />
              {lockMutation.isPending ? t('payroll.detail.locking') : t('payroll.detail.lockMarkPaid')}
            </button>
          )}
          {canPay && canDownloadPayslips(run.status) && (
            <>
              <button
                onClick={() => downloadPaymentFile(id!)}
                className="flex items-center gap-2 bg-white border border-gray-200 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 text-sm font-medium"
              >
                <Download className="w-4 h-4" /> {t('payroll.detail.paymentFile')}
              </button>
              <button
                onClick={() => journalMutation.mutate()}
                disabled={journalMutation.isPending}
                className="flex items-center gap-2 bg-white border border-gray-200 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 disabled:opacity-50 text-sm font-medium"
              >
                <FileText className="w-4 h-4" />
                {journalMutation.isPending ? t('common.loading') : t('payroll.detail.journalPreview')}
              </button>
            </>
          )}
          {showCancel && (
            <button
              onClick={() => {
                const reason = prompt(t('payroll.detail.cancelPrompt'))?.trim();
                if (reason) cancelMutation.mutate(reason);
              }}
              disabled={cancelMutation.isPending}
              className="flex items-center gap-2 bg-white border border-red-200 text-red-600 px-4 py-2 rounded-lg hover:bg-red-50 disabled:opacity-50 text-sm font-medium"
            >
              <Ban className="w-4 h-4" />
              {cancelMutation.isPending ? t('payroll.detail.cancelling') : t('payroll.detail.cancelRun')}
            </button>
          )}
          <span className={`px-3 py-1 rounded-full text-sm font-medium ${
            run.status === 'paid' ? 'bg-emerald-50 text-emerald-700' :
            run.status === 'approved' ? 'bg-green-50 text-green-700' :
            run.status === 'pending_approval' ? 'bg-blue-50 text-blue-700' :
            'bg-gray-100 text-gray-900'
          }`}>
            {payrollStatusLabel(run.status)}
          </span>
        </div>
      </div>
      {lifecycleError && (
        <div className="mb-6 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
          {lifecycleError}
        </div>
      )}
      {run.status === 'paid' && (
        <div className="mb-6 rounded-lg border border-emerald-100 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
          {t('payroll.detail.paidNotice')}
        </div>
      )}
      {run.status === 'pending_approval' && (
        <div className="mb-6 rounded-lg border border-blue-100 bg-blue-50 px-4 py-3 text-sm text-blue-700">
          {t('payroll.detail.pendingNotice')}
        </div>
      )}
      {run.status === 'cancelled' && (
        <div className="mb-6 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
          {t('payroll.detail.cancelledNotice', {
            reason: run.cancel_reason ? t('payroll.detail.cancelledReason', { reason: run.cancel_reason }) : '',
            date: run.cancelled_at ? t('payroll.detail.cancelledOn', { date: formatDate(run.cancelled_at) }) : '',
          })}
        </div>
      )}

      <PayrollAuditTimeline logs={auditLogs} />

      {/* Summary Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.totals.gross')}</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_gross)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.totals.net')}</p>
          <p className="text-lg font-bold mt-1 text-green-600">{formatMYR(run.total_net)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.totals.employerCost')}</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_employer_cost)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.totals.pcb')}</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_pcb)}</p>
        </div>
      </div>

      {/* Statutory Summary */}
      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.preview.epfFull')}</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_epf_employee)} + {formatMYR(run.total_epf_employer)}
          </p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.preview.socsoFull')}</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_socso_employee)} + {formatMYR(run.total_socso_employer)}
          </p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">{t('payroll.preview.eisFull')}</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_eis_employee)} + {formatMYR(run.total_eis_employer)}
          </p>
        </div>
      </div>

      {/* Employee Details Table */}
      <div className="bg-white rounded-2xl shadow border border-gray-200 overflow-hidden">
        <div className="px-6 py-4 border-b border-gray-200">
          <h2 className="font-semibold">{t('payroll.detail.breakdownTitle')}</h2>
          <p className="mt-1 text-sm text-gray-500">
            {t('payroll.detail.breakdownBody')}
            {canEditPcb && ` ${t('payroll.detail.breakdownPcbHint')}`}
          </p>
          {pcbError && (
            <div className="mt-3 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
              {pcbError}
            </div>
          )}
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 border-b border-gray-200">
              <tr>
                <th className="text-left px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('employees.columns.employee')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.basic')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.allowances')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.ot')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.gross')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">EPF</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">SOCSO</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">EIS</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">PCB</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.deductions')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.claims')}</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">{t('payroll.cols.net')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {items.map((item) => (
                <tr key={item.employee_id} className="hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <button
                      type="button"
                      onClick={() => setBreakdownEmployeeId(item.employee_id)}
                      className="text-left"
                    >
                      <div className="text-sm font-medium text-gray-900 underline decoration-gray-300 underline-offset-2 hover:decoration-gray-900">
                        {item.employee_name}
                      </div>
                      <div className="text-xs text-gray-400">{item.employee_number}</div>
                    </button>
                  </td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.basic_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_allowances > 0 ? formatMYR(item.total_allowances) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_overtime > 0 ? formatMYR(item.total_overtime) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.gross_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.epf_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.socso_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.eis_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">
                    {editingPcbEmployeeId === item.employee_id ? (
                      <div className="flex min-w-40 items-center justify-end gap-2">
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          value={pcbInput}
                          onChange={(e) => setPcbInput(e.target.value)}
                          className="w-24 rounded-lg border border-gray-200 px-2 py-1 text-right text-sm outline-none focus:border-gray-400"
                        />
                        <button
                          type="button"
                          onClick={() => savePcb(item.employee_id)}
                          disabled={updatePcbMutation.isPending}
                          className="rounded-lg p-1.5 text-green-600 hover:bg-green-50 disabled:opacity-50"
                          title={t('payroll.detail.savePcb')}
                        >
                          <Save className="w-4 h-4" />
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setEditingPcbEmployeeId(null);
                            setPcbInput('');
                            setPcbError('');
                          }}
                          className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100"
                          title={t('payroll.detail.cancelPcbEdit')}
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ) : (
                      <div className="flex items-center justify-end gap-2">
                        <span>{formatMYR(item.pcb_amount)}</span>
                        {canEditPcb && (
                          <button
                            type="button"
                            onClick={() => startEditPcb(item.employee_id, item.pcb_amount)}
                            className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100 hover:text-black"
                            title={t('payroll.detail.editPcb')}
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-3 text-sm text-right text-red-600">{formatMYR(item.total_deductions)}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_claims > 0 ? <span className="text-blue-600">{formatMYR(item.total_claims)}</span> : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right font-bold text-green-600">{formatMYR(item.net_salary)}</td>
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
              <div className="mb-4 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
                {t('payroll.detail.journalUnbalanced', {
                  debits: formatMYR(journal.total_debits),
                  credits: formatMYR(journal.total_credits),
                })}
              </div>
            )}
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-gray-400 border-b border-gray-100">
                  <th className="py-2 font-medium">{t('payroll.detail.journalCols.account')}</th>
                  <th className="py-2 font-medium">{t('payroll.detail.journalCols.department')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.detail.journalCols.debit')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.detail.journalCols.credit')}</th>
                </tr>
              </thead>
              <tbody>
                {journal.lines.map((line, i) => (
                  <tr key={i} className="border-b border-gray-50 last:border-0">
                    <td className="py-2">
                      <span className="font-medium">{line.account_code}</span>{' '}
                      <span className="text-gray-600">{line.account_name}</span>
                      {line.memo && <div className="text-xs text-gray-400">{line.memo}</div>}
                    </td>
                    <td className="py-2 text-gray-500">{line.department ?? '—'}</td>
                    <td className="py-2 text-right tabular-nums">
                      {line.side === 'debit' ? formatMYR(line.amount) : ''}
                    </td>
                    <td className="py-2 text-right tabular-nums">
                      {line.side === 'credit' ? formatMYR(line.amount) : ''}
                    </td>
                  </tr>
                ))}
                <tr className="border-t border-gray-200 font-semibold">
                  <td className="py-2" colSpan={2}>{t('payroll.detail.journalTotals')}</td>
                  <td className="py-2 text-right tabular-nums">{formatMYR(journal.total_debits)}</td>
                  <td className="py-2 text-right tabular-nums">{formatMYR(journal.total_credits)}</td>
                </tr>
              </tbody>
            </table>
            <ul className="mt-4 list-disc pl-5 text-xs text-gray-500 space-y-1">
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
    <div className="mt-6 rounded-2xl border border-gray-200 bg-white p-4 shadow">
      <h2 className="mb-1 text-sm font-semibold text-gray-900">{t('payroll.detail.basisTitle')}</h2>
      <p className="mb-3 text-xs text-gray-500">
        {t('payroll.detail.basisBody', { date: formatDate(snapshot.effective_date) })}
      </p>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-xs uppercase tracking-wide text-gray-400">
            <tr>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.domain')}</th>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.dataset')}</th>
              <th className="py-1 pr-4">{t('payroll.detail.basisCols.sourceVersion')}</th>
              <th className="py-1">{t('payroll.detail.basisCols.effective')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {snapshot.statutory_rule_sets.map((ruleSet) => (
              <tr key={ruleSet.rule_set_id}>
                <td className="py-1.5 pr-4 font-medium uppercase">{ruleSet.rule_code}</td>
                <td className="py-1.5 pr-4 text-gray-600">{ruleSet.dataset_key}</td>
                <td className="py-1.5 pr-4 text-gray-600">{ruleSet.source_version ?? '—'}</td>
                <td className="py-1.5 text-gray-600">
                  {formatDate(ruleSet.effective_from)} → {ruleSet.effective_to ? formatDate(ruleSet.effective_to) : t('payroll.detail.effectiveOpen')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-3 text-xs text-gray-500">
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
  const visibleLogs = logs.slice(0, 6);

  return (
    <div className="mb-6 bg-white rounded-2xl shadow border border-gray-200 p-4">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-900">{t('payroll.detail.auditTitle')}</h2>
        <span className="text-xs text-gray-400">{t('payroll.detail.auditRecords', { count: logs.length })}</span>
      </div>
      {visibleLogs.length === 0 ? (
        <p className="text-sm text-gray-400">{t('payroll.detail.auditEmpty')}</p>
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {visibleLogs.map((log) => (
            <div key={log.id} className="rounded-lg border border-gray-100 px-3 py-2">
              <div className="mb-1 flex items-center justify-between gap-2">
                <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${getAuditActionClass(log.action)}`}>
                  {formatAuditAction(log.action, t)}
                </span>
                <span className="inline-flex items-center gap-1 text-xs text-gray-400">
                  <Clock className="h-3 w-3" />
                  {formatDateTime(log.created_at)}
                </span>
              </div>
              <p className="truncate text-sm text-gray-700">{log.description || t('payroll.detail.auditEventFallback')}</p>
              <p className="mt-0.5 truncate text-xs text-gray-400">{log.user_full_name || log.user_email || t('payroll.detail.system')}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function getAuditActionClass(action: string) {
  const classes: Record<string, string> = {
    process: 'bg-indigo-50 text-indigo-700',
    approve: 'bg-emerald-50 text-emerald-700',
    submit_approval: 'bg-blue-50 text-blue-700',
    return_changes: 'bg-amber-50 text-amber-700',
    lock: 'bg-gray-900 text-white',
    update: 'bg-blue-50 text-blue-700',
    delete: 'bg-red-50 text-red-700',
  };
  return classes[action] ?? 'bg-gray-100 text-gray-700';
}

function formatAuditAction(action: string, t: TFunction) {
  const key = `payroll.detail.auditActions.${action}`;
  return i18n.exists(key) ? t(key) : action.replace(/_/g, ' ').replace(/^\w/, (char) => char.toUpperCase());
}
