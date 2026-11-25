import { useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  CheckCircle,
  Clock,
  ExternalLink,
  Filter,
  Image,
  Paperclip,
  Plus,
  Trash2,
  X,
  XCircle,
} from 'lucide-react';
import {
  approveClaim,
  approveLeave,
  approveOvertime,
  cancelClaim,
  cancelLeaveRequest,
  cancelOvertimeRequest,
  deleteClaim,
  deleteLeaveRequest,
  deleteOvertimeRequest,
  getClaims,
  getLeaveRequests,
  getOvertimeRequests,
  rejectClaim,
  rejectLeave,
  rejectOvertime,
  type ClaimWithEmployee,
  type LeaveRequestWithEmployee,
  type OvertimeWithEmployee,
} from '@/api/approvals';
import { getLeaveTypes } from '@/api/portal';
import { AttachmentLink, AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { useAuth } from '@/context/AuthContext';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';
import { formatDate, getErrorMessage, isImageUrl } from '@/lib/utils';
import { formatMYR } from '@/lib/format';
import { ActionButtons, RequestStatusBadge, SummaryField } from './ApprovalShared';
import {
  canCancelClaim,
  canCancelLeave,
  canCancelOvertime,
  canDeleteClaim,
  canDeleteLeave,
  canDeleteOvertime,
  canEditClaim,
  canEditLeave,
  canEditOvertime,
} from './approvalRules';
import { LeaveCrudModal } from './LeaveCrudModal';
import { ClaimCrudModal } from './ClaimCrudModal';
import { OvertimeCrudModal } from './OvertimeCrudModal';

const fmt = formatMYR;
const statusBadge = (status: string) => <RequestStatusBadge status={status} />;

/** Rows per page in the approval inboxes. */
const PER_PAGE = 20;

export function Approvals() {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const [searchParams, setSearchParams] = useSearchParams();
  const { user } = useAuth();
  const { can } = usePermissions();
  const canManageApprovals = can('manage_approvals');
  const urlTabParam = searchParams.get('tab');
  const resolvedUrlTab: 'leave' | 'claims' | 'overtime' =
    urlTabParam === 'claims' || urlTabParam === 'overtime' ? urlTabParam : 'leave';
  const [tab, setTabState] = useState<'leave' | 'claims' | 'overtime'>(resolvedUrlTab);
  const setTab = (next: 'leave' | 'claims' | 'overtime') => {
    setTabState(next);
    const params = new URLSearchParams(searchParams);
    if (next === 'leave') params.delete('tab');
    else params.set('tab', next);
    setSearchParams(params, { replace: true });
  };
  useEffect(() => {
    setTabState((current) => (current === resolvedUrlTab ? current : resolvedUrlTab));
  }, [resolvedUrlTab]);
  const [statusFilter, setStatusFilter] = useState<string>('pending');
  const [page, setPageState] = useState(1);
  const setPage = (nextPage: number) => {
    setPageState(nextPage);
    setSelectedApprovalIds([]);
  };
  const [reviewNotes, setReviewNotes] = useState<Record<string, string>>({});
  const [leaveEditor, setLeaveEditor] = useState<LeaveRequestWithEmployee | null>(null);
  const [claimEditor, setClaimEditor] = useState<ClaimWithEmployee | null>(null);
  const [overtimeEditor, setOvertimeEditor] = useState<OvertimeWithEmployee | null>(null);
  const [selectedApprovalIds, setSelectedApprovalIds] = useState<string[]>([]);
  const [bulkError, setBulkError] = useState('');
  const [showLeaveModal, setShowLeaveModal] = useState(false);
  const [showClaimModal, setShowClaimModal] = useState(false);
  const [showOvertimeModal, setShowOvertimeModal] = useState(false);
  const queryClient = useQueryClient();

  // Maker-checker. The server is the gate — `approve_claim` / `approve_leave` /
  // `approve_overtime` return 403 when the reviewer is the subject, resolving
  // the link from the `users` row rather than from the token. This only stops
  // the UI offering an action that cannot succeed, and must never be mistaken
  // for the control itself. `super_admin` carries the documented override.
  const isSelfApproval = (employeeId: string) =>
    Boolean(user?.employee_id) &&
    user?.employee_id === employeeId &&
    !(user?.roles ?? []).includes('super_admin');

  const { data: leaveTypes = [] } = useQuery({
    queryKey: ['approval-leave-types'],
    queryFn: getLeaveTypes,
    enabled: canManageApprovals && (tab === 'leave' || showLeaveModal),
  });

  // Server-side paging. The inboxes previously fetched a bare array capped at
  // 100 rows and let DataTable slice it client-side, which rendered the cap as
  // the queue total — so a manager with more than 100 requests saw "100 of 100"
  // and could never reach the rest. Since the sort is newest-first, the rows
  // being hidden were the oldest and most overdue.
  const leaveQuery = useQuery({
    queryKey: ['approvals-leave', statusFilter, page],
    queryFn: () => getLeaveRequests({ status: statusFilter || undefined, page, per_page: PER_PAGE }),
    enabled: tab === 'leave',
    placeholderData: keepPreviousData,
  });

  const claimsQuery = useQuery({
    queryKey: ['approvals-claims', statusFilter, page],
    queryFn: () => getClaims({ status: statusFilter || undefined, page, per_page: PER_PAGE }),
    enabled: tab === 'claims',
    placeholderData: keepPreviousData,
  });

  const overtimeQuery = useQuery({
    queryKey: ['approvals-overtime', statusFilter, page],
    queryFn: () =>
      getOvertimeRequests({ status: statusFilter || undefined, page, per_page: PER_PAGE }),
    enabled: tab === 'overtime',
    placeholderData: keepPreviousData,
  });

  const refreshPendingCount = () =>
    queryClient.invalidateQueries({ queryKey: ['approvals-pending-count'] });
  const refreshLeave = () => {
    void queryClient.invalidateQueries({ queryKey: ['approvals-leave'] });
    void refreshPendingCount();
  };
  const refreshClaims = () => {
    void queryClient.invalidateQueries({ queryKey: ['approvals-claims'] });
    void refreshPendingCount();
  };
  const refreshOvertime = () => {
    void queryClient.invalidateQueries({ queryKey: ['approvals-overtime'] });
    void refreshPendingCount();
  };

  const approveLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveLeave(id, notes),
    onSuccess: () => {
      refreshLeave();
      toast.success(t('approvals.approvedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const rejectLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectLeave(id, notes),
    onSuccess: () => {
      refreshLeave();
      toast.success(t('approvals.rejectedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const approveClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveClaim(id, notes),
    onSuccess: () => {
      refreshClaims();
      toast.success(t('approvals.approvedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const rejectClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectClaim(id, notes),
    onSuccess: () => {
      refreshClaims();
      toast.success(t('approvals.rejectedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const approveOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveOvertime(id, notes),
    onSuccess: () => {
      refreshOvertime();
      toast.success(t('approvals.approvedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const rejectOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectOvertime(id, notes),
    onSuccess: () => {
      refreshOvertime();
      toast.success(t('approvals.rejectedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const cancelLeaveM = useMutation({
    mutationFn: cancelLeaveRequest,
    onSuccess: () => {
      refreshLeave();
      toast.success(t('approvals.cancelledSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const cancelClaimM = useMutation({
    mutationFn: cancelClaim,
    onSuccess: () => {
      refreshClaims();
      toast.success(t('approvals.cancelledSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const cancelOvertimeM = useMutation({
    mutationFn: cancelOvertimeRequest,
    onSuccess: () => {
      refreshOvertime();
      toast.success(t('approvals.cancelledSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const deleteLeaveM = useMutation({
    mutationFn: deleteLeaveRequest,
    onSuccess: () => {
      refreshLeave();
      toast.success(t('approvals.deletedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const deleteClaimM = useMutation({
    mutationFn: deleteClaim,
    onSuccess: () => {
      refreshClaims();
      toast.success(t('approvals.deletedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const deleteOvertimeM = useMutation({
    mutationFn: deleteOvertimeRequest,
    onSuccess: () => {
      refreshOvertime();
      toast.success(t('approvals.deletedSuccess'));
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('approvals.actionFailed'))),
  });

  const refreshAll = () => {
    refreshLeave();
    refreshClaims();
    refreshOvertime();
  };

  const cancelActionFor = (target: typeof tab) =>
    target === 'leave' ? cancelLeaveRequest : target === 'claims' ? cancelClaim : cancelOvertimeRequest;

  const deleteActionFor = (target: typeof tab) =>
    target === 'leave' ? deleteLeaveRequest : target === 'claims' ? deleteClaim : deleteOvertimeRequest;

  // `runBulk` reports a partial failure as a value, so the refetch below runs on
  // every outcome and the rows that failed stay selected — a retry then hits
  // exactly those. Under the old `Promise.all` a single rejection skipped
  // `onSuccess` entirely: nothing invalidated, nothing was deselected, and the
  // rows that had succeeded went on showing their stale status.
  const bulkCancelM = useMutation({
    mutationFn: ({ ids, target }: { ids: string[]; target: typeof tab }) =>
      runBulk(ids, cancelActionFor(target)),
    onSuccess: (outcome) => {
      setSelectedApprovalIds(outcome.failed.map((failure) => failure.id));
      const msg = summarizeBulkFailure(outcome, 'cancelled');
      setBulkError(msg);
      if (!msg) toast.success(t('approvals.cancelledSuccess'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('approvals.bulkCancelFailed'))),
    onSettled: refreshAll,
  });

  const bulkDeleteM = useMutation({
    mutationFn: ({ ids, target }: { ids: string[]; target: typeof tab }) =>
      runBulk(ids, deleteActionFor(target)),
    onSuccess: (outcome) => {
      setSelectedApprovalIds(outcome.failed.map((failure) => failure.id));
      const msg = summarizeBulkFailure(outcome, 'deleted');
      setBulkError(msg);
      if (!msg) toast.success(t('approvals.deletedSuccess'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('approvals.bulkDeleteFailed'))),
    onSettled: refreshAll,
  });

  const otTypeLabel = (type: string) => t(`enums.otType.${type}`, { defaultValue: type });

  const otTypeMultiplier = (type: string) => {
    const multipliers: Record<string, string> = {
      normal: '1.5x',
      rest_day: '2.0x',
      public_holiday: '3.0x',
    };
    return multipliers[type] || '';
  };

  const openCreateModal = () => {
    if (tab === 'leave') {
      setLeaveEditor(null);
      setShowLeaveModal(true);
      return;
    }
    if (tab === 'claims') {
      setClaimEditor(null);
      setShowClaimModal(true);
      return;
    }
    setOvertimeEditor(null);
    setShowOvertimeModal(true);
  };

  const activeRows = tab === 'leave'
    ? leaveQuery.data?.data ?? []
    : tab === 'claims'
      ? claimsQuery.data?.data ?? []
      : overtimeQuery.data?.data ?? [];
  const selectedRows = activeRows.filter((row) => selectedApprovalIds.includes(row.id));
  const selectedCancelableIds = selectedRows
    .filter((row) => ['pending', 'approved', 'rejected'].includes(row.status))
    .map((row) => row.id);
  const selectedDeletableIds = selectedRows
    .filter((row) => (tab === 'claims' ? ['draft', 'cancelled'].includes(row.status) : row.status === 'cancelled'))
    .map((row) => row.id);

  const leaveColumns: Column<LeaveRequestWithEmployee>[] = [
    {
      key: 'employee',
      header: t('common.employee'),
      render: (request) => (
        <div className="min-w-0">
          <div className="font-semibold text-gray-900 dark:text-slate-100">{request.employee_name}</div>
          <div className="text-xs text-gray-500 dark:text-slate-400">{request.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    { key: 'type', header: t('approvals.leaveType'), render: (request) => request.leave_type_name || '\u2014', primary: true },
    { key: 'period', header: t('approvals.period'), render: (request) => <span className="text-gray-600 dark:text-slate-300 whitespace-nowrap">{request.start_date} {'\u2192'} {request.end_date}</span> },
    { key: 'days', header: t('approvals.days'), render: (request) => <span className="tabular-nums">{Number(request.days)}</span> },
    { key: 'reason', header: t('common.reason'), render: (request) => <span className="text-gray-500 dark:text-slate-400 max-w-[180px] truncate block">{request.reason || '\u2014'}</span> },
    {
      key: 'attachment',
      header: t('approvals.attachment'),
      render: (request) =>
        request.attachment_url ? (
          <span className="inline-flex items-center gap-1 text-gray-900 dark:text-slate-100 text-sm">
            <Paperclip className="w-3 h-3 shrink-0" />
            <span className="truncate max-w-[80px]">{request.attachment_name || t('approvals.file')}</span>
          </span>
        ) : (
          <span className="text-gray-300 dark:text-slate-600">{'\u2014'}</span>
        ),
    },
    { key: 'status', header: t('common.status'), render: (request) => statusBadge(request.status) },
  ];

  const claimColumns: Column<ClaimWithEmployee>[] = [
    {
      key: 'employee',
      header: t('common.employee'),
      render: (claim) => (
        <div className="min-w-0">
          <div className="font-semibold text-gray-900 dark:text-slate-100">{claim.employee_name}</div>
          <div className="text-xs text-gray-500 dark:text-slate-400">{claim.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    {
      key: 'title',
      header: t('common.title'),
      render: (claim) => (
        <div className="min-w-0">
          <div className="font-medium text-gray-900 dark:text-slate-100">{claim.title}</div>
          {claim.description && <div className="text-xs text-gray-500 dark:text-slate-400 truncate max-w-[180px]">{claim.description}</div>}
        </div>
      ),
      primary: true,
    },
    { key: 'category', header: t('common.category'), render: (claim) => claim.category || '\u2014' },
    { key: 'amount', header: t('common.amount'), align: 'right', render: (claim) => <span className="font-semibold tabular-nums whitespace-nowrap">{fmt(claim.amount)}</span> },
    { key: 'date', header: t('common.date'), render: (claim) => <span className="text-gray-600 dark:text-slate-300 whitespace-nowrap">{claim.expense_date}</span> },
    {
      key: 'receipt',
      header: t('approvals.receipt'),
      render: (claim) =>
        claim.receipt_url ? (
          <AttachmentLink
            url={claim.receipt_url}
            className="inline-flex items-center gap-1 text-gray-900 dark:text-slate-100 hover:text-black dark:hover:text-white text-sm"
          >
            {isImageUrl(claim.receipt_url) ? <Image className="w-3 h-3 shrink-0" /> : <Paperclip className="w-3 h-3 shrink-0" />}
            <span className="truncate max-w-[80px]">{claim.receipt_file_name || t('approvals.receipt')}</span>
            <ExternalLink className="w-3 h-3 shrink-0" />
          </AttachmentLink>
        ) : (
          <span className="text-gray-300 dark:text-slate-600">{'\u2014'}</span>
        ),
    },
    { key: 'status', header: t('common.status'), render: (claim) => statusBadge(claim.status) },
  ];

  const overtimeColumns: Column<OvertimeWithEmployee>[] = [
    {
      key: 'employee',
      header: t('common.employee'),
      render: (overtime) => (
        <div className="min-w-0">
          <div className="font-semibold text-gray-900 dark:text-slate-100">{overtime.employee_name}</div>
          <div className="text-xs text-gray-500 dark:text-slate-400">{overtime.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    { key: 'date', header: t('common.date'), render: (overtime) => <span className="text-gray-700 dark:text-slate-200 whitespace-nowrap">{overtime.ot_date}</span>, primary: true },
    {
      key: 'type',
      header: t('common.type'),
      render: (overtime) => (
        <span
          className={`text-xs px-2 py-0.5 rounded-full font-medium whitespace-nowrap ${
            overtime.ot_type === 'public_holiday'
              ? 'bg-red-100 text-red-700 dark:bg-red-500/15 dark:text-red-300'
              : overtime.ot_type === 'rest_day'
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-500/15 dark:text-blue-300'
                : 'bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300'
          }`}
        >
          {otTypeLabel(overtime.ot_type)} ({otTypeMultiplier(overtime.ot_type)})
        </span>
      ),
    },
    {
      key: 'time',
      header: t('common.time'),
      render: (overtime) => <span className="text-gray-500 dark:text-slate-400 whitespace-nowrap tabular-nums">{overtime.start_time?.slice(0, 5)} {'\u2014'} {overtime.end_time?.slice(0, 5)}</span>,
    },
    { key: 'hours', header: t('common.hours'), align: 'right', render: (overtime) => <span className="font-semibold tabular-nums whitespace-nowrap">{Number(overtime.hours)}h</span> },
    { key: 'reason', header: t('common.reason'), render: (overtime) => <span className="text-gray-500 dark:text-slate-400 max-w-[180px] truncate block">{overtime.reason || '\u2014'}</span> },
    { key: 'status', header: t('common.status'), render: (overtime) => statusBadge(overtime.status) },
  ];

  const headerButtonLabel = useMemo(() => {
    if (tab === 'leave') return t('approvals.addLeave');
    if (tab === 'claims') return t('approvals.addClaim');
    return t('approvals.addOvertime');
  }, [tab, t]);

  const statusOptions = tab === 'claims'
    ? ['pending', 'draft', 'approved', 'rejected', 'cancelled', '']
    : ['pending', 'approved', 'rejected', 'cancelled', ''];

  const activeQuery = tab === 'leave' ? leaveQuery : tab === 'claims' ? claimsQuery : overtimeQuery;

  return (
    <>
      <div className="space-y-6">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="page-header">
            <h1 className="page-title">{t('approvals.title')}</h1>
            <p className="page-subtitle">{t('approvals.subtitle')}</p>
          </div>
          {canManageApprovals && (
            <button type="button" onClick={openCreateModal} className="btn-primary w-full sm:w-auto">
              <Plus className="w-4 h-4" /> {headerButtonLabel}
            </button>
          )}
        </div>

        <div className="flex gap-1 border-b border-gray-200 dark:border-slate-800 overflow-x-auto">
          {(['leave', 'claims', 'overtime'] as const).map((itemTab) => (
            <button
              key={itemTab}
              type="button"
              aria-pressed={tab === itemTab}
              onClick={() => {
                setTab(itemTab);
                setStatusFilter('pending');
                setSelectedApprovalIds([]);
                setPageState(1);
                setBulkError('');
              }}
              className={`px-5 py-2.5 text-sm font-medium border-b-2 transition-all-fast whitespace-nowrap shrink-0 ${
                tab === itemTab
                  ? 'border-[var(--accent-1)] text-gray-900 dark:text-slate-100'
                  : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-slate-400 dark:hover:text-slate-200'
              }`}
            >
              {t(`approvals.tabs.${itemTab}`)}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Filter className="w-4 h-4 text-gray-400 dark:text-slate-500 shrink-0" />
          {statusOptions.map((status) => (
            <button
              key={status || 'all'}
              type="button"
              aria-pressed={statusFilter === status}
              onClick={() => {
                setSelectedApprovalIds([]);
                setBulkError('');
                setStatusFilter(status);
                setPageState(1);
              }}
              className={`px-3.5 py-2 sm:py-1.5 text-xs font-medium rounded-full transition-all-fast ${
                statusFilter === status
                  ? 'bg-[var(--accent-1)] text-white shadow-sm'
                  : 'bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-300 border border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600'
              }`}
            >
              {status ? t(`enums.requestStatus.${status}`) : t('common.all')}
            </button>
          ))}
        </div>

        {canManageApprovals && selectedApprovalIds.length > 0 && (
          <div className="space-y-2">
            <div className="flex flex-col gap-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex flex-wrap items-center gap-3">
                <span className="text-sm font-medium text-gray-700 dark:text-slate-200">{t('common.selectedCount', { count: selectedApprovalIds.length })}</span>
                <button
                  type="button"
                  onClick={() => {
                    setSelectedApprovalIds([]);
                    setBulkError('');
                  }}
                  className="text-xs text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-200 underline"
                >
                  {t('ux.table.clearSelection')}
                </button>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('approvals.bulkCancelConfirm', { count: selectedCancelableIds.length }),
                      confirmLabel: t('common.cancelSelected'),
                      variant: 'warning',
                    });
                    if (ok) {
                      setBulkError('');
                      bulkCancelM.mutate({ ids: selectedCancelableIds, target: tab });
                    }
                  }}
                  disabled={selectedCancelableIds.length === 0 || bulkCancelM.isPending}
                  className="btn-secondary !py-2 text-sm disabled:opacity-50"
                >
                  <X className="w-4 h-4" />
                  {bulkCancelM.isPending ? t('common.cancelling') : t('common.cancelSelected')}
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('approvals.bulkDeleteConfirm', { count: selectedDeletableIds.length }),
                      variant: 'danger',
                    });
                    if (ok) {
                      setBulkError('');
                      bulkDeleteM.mutate({ ids: selectedDeletableIds, target: tab });
                    }
                  }}
                  disabled={selectedDeletableIds.length === 0 || bulkDeleteM.isPending}
                  className="btn-secondary !py-2 text-sm text-red-600 dark:text-red-400 border-red-200 dark:border-red-500/30 hover:!bg-red-50 dark:hover:!bg-red-500/15 disabled:opacity-50"
                >
                  <Trash2 className="w-4 h-4" />
                  {bulkDeleteM.isPending ? t('common.deleting') : t('common.deleteSelected')}
                </button>
              </div>
            </div>
            {/* The rows still selected are precisely the ones that failed, so
                the banner and the selection describe the same set. */}
            {bulkError && (
              <div role="alert" className="rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300">
                {bulkError}
              </div>
            )}
          </div>
        )}

        {activeQuery.isError ? (
          <EmptyState
            icon={AlertTriangle}
            title={t('approvals.loadFailed')}
            description={getErrorMessage(activeQuery.error, t('approvals.loadFailed'))}
            action={
              <button type="button" onClick={() => activeQuery.refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        ) : tab === 'leave' ? (
          <DataTable
            columns={leaveColumns}
            data={leaveQuery.data?.data ?? []}
            total={leaveQuery.data?.total}
            page={page}
            onPageChange={setPage}
            perPage={PER_PAGE}
            isLoading={leaveQuery.isLoading}
            emptyMessage={t('approvals.noLeaveRequests')}
            emptyIcon={<Clock className="w-8 h-8 text-gray-300 dark:text-slate-600" />}
            selectable={canManageApprovals}
            selectedRowKeys={selectedApprovalIds}
            onSelectedRowKeysChange={setSelectedApprovalIds}
            rowKey={(request) => request.id}
            renderActions={
              canManageApprovals
                ? (request) => (
                    <ActionButtons
                      onEdit={canEditLeave(request) ? () => {
                        setLeaveEditor(request);
                        setShowLeaveModal(true);
                      } : undefined}
                      onCancel={canCancelLeave(request) ? async () => {
                        const ok = await confirmDialog({
                          description: t('approvals.cancelLeaveConfirm'),
                          confirmLabel: t('common.cancel'),
                          variant: 'warning',
                        });
                        if (ok) {
                          cancelLeaveM.mutate(request.id);
                        }
                      } : undefined}
                      onDelete={canDeleteLeave(request) ? async () => {
                        const ok = await confirmDialog({
                          description: t('approvals.deleteLeaveConfirm'),
                          variant: 'danger',
                        });
                        if (ok) {
                          deleteLeaveM.mutate(request.id);
                        }
                      } : undefined}
                    />
                  )
                : undefined
            }
            summaryTitle={(request) => t('approvals.summaryTitle', { name: request.employee_name, type: t('approvals.leaveRequest') })}
            renderSummary={(request) => (
              <div className="space-y-5">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-11 h-11 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-900 dark:text-slate-100 font-semibold text-sm shrink-0">
                      {(request.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div className="min-w-0">
                      <div className="font-semibold text-lg text-gray-900 dark:text-slate-100 break-words">{request.employee_name}</div>
                      <div className="text-sm text-gray-500 dark:text-slate-400">{request.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(request.status)}
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 bg-gray-50 dark:bg-slate-800/70 rounded-lg p-4">
                  <SummaryField label={t('approvals.leaveType')} value={request.leave_type_name || '\u2014'} />
                  <SummaryField label={t('approvals.duration')} value={t('approvals.daysCount', { count: Number(request.days) })} />
                  <SummaryField label={t('common.startDate')} value={formatDate(request.start_date)} />
                  <SummaryField label={t('common.endDate')} value={formatDate(request.end_date)} />
                </div>

                {request.reason && <SummaryField label={t('common.reason')} value={request.reason} />}

                {request.attachment_url && (
                  <div>
                    <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide mb-1">{t('approvals.attachment')}</p>
                    <AttachmentPreview url={request.attachment_url} name={request.attachment_name} />
                  </div>
                )}

                {request.review_notes && (
                  <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/30 rounded-lg p-4">
                    <SummaryField label={t('approvals.reviewNotes')} value={request.review_notes} />
                    {request.reviewed_at && <p className="text-xs text-gray-500 dark:text-slate-400 mt-1">{t('approvals.reviewedOn', { date: formatDate(request.reviewed_at) })}</p>}
                  </div>
                )}

                {canManageApprovals && request.status === 'pending' && (
                  <div>
                    <label htmlFor={`review-leave-${request.id}`} className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide block mb-1">
                      {t('approvals.reviewNotes')}
                    </label>
                    <textarea
                      id={`review-leave-${request.id}`}
                      placeholder={t('approvals.notesPlaceholder')}
                      value={reviewNotes[request.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [request.id]: event.target.value }))}
                      rows={2}
                      className="form-input resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={
              canManageApprovals
                ? (request, close) =>
                    request.status === 'pending' ? (
                      <div className="flex flex-wrap items-center justify-end gap-3">
                        <button
                          type="button"
                          onClick={() => rejectLeaveM.mutate({ id: request.id, notes: reviewNotes[request.id] }, { onSuccess: close })}
                          disabled={rejectLeaveM.isPending || approveLeaveM.isPending}
                          className="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/40 text-red-600 dark:text-red-400 text-sm font-medium rounded-lg hover:bg-red-50 dark:hover:bg-red-500/15 hover:border-red-300 disabled:opacity-50 transition-colors"
                        >
                          <XCircle className="w-4 h-4" />
                          {rejectLeaveM.isPending ? t('common.rejecting') : t('common.reject')}
                        </button>
                        {isSelfApproval(request.employee_id) ? (
                          <p className="text-sm text-gray-500 dark:text-slate-400 max-w-xs text-right">
                            {t('approvals.selfApprovalLeave')}
                          </p>
                        ) : (
                          <button
                            type="button"
                            onClick={() => approveLeaveM.mutate({ id: request.id, notes: reviewNotes[request.id] }, { onSuccess: close })}
                            disabled={approveLeaveM.isPending || rejectLeaveM.isPending}
                            className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                          >
                            <CheckCircle className="w-4 h-4" />
                            {approveLeaveM.isPending ? t('common.approving') : t('common.approve')}
                          </button>
                        )}
                      </div>
                    ) : null
                : undefined
            }
          />
        ) : tab === 'claims' ? (
          <DataTable
            columns={claimColumns}
            data={claimsQuery.data?.data ?? []}
            total={claimsQuery.data?.total}
            page={page}
            onPageChange={setPage}
            perPage={PER_PAGE}
            isLoading={claimsQuery.isLoading}
            emptyMessage={t('approvals.noClaims')}
            emptyIcon={<Clock className="w-8 h-8 text-gray-300 dark:text-slate-600" />}
            selectable={canManageApprovals}
            selectedRowKeys={selectedApprovalIds}
            onSelectedRowKeysChange={setSelectedApprovalIds}
            rowKey={(claim) => claim.id}
            renderActions={
              canManageApprovals
                ? (claim) => (
                    <ActionButtons
                      onEdit={canEditClaim(claim) ? () => {
                        setClaimEditor(claim);
                        setShowClaimModal(true);
                      } : undefined}
                      onCancel={canCancelClaim(claim) ? async () => {
                        const ok = await confirmDialog({
                          description: t('approvals.cancelClaimConfirm'),
                          confirmLabel: t('common.cancel'),
                          variant: 'warning',
                        });
                        if (ok) {
                          cancelClaimM.mutate(claim.id);
                        }
                      } : undefined}
                      onDelete={canDeleteClaim(claim) ? async () => {
                        const message = claim.status === 'cancelled'
                          ? t('approvals.deleteCancelledClaimConfirm')
                          : t('approvals.deleteDraftClaimConfirm');
                        const ok = await confirmDialog({
                          description: message,
                          variant: 'danger',
                        });
                        if (ok) {
                          deleteClaimM.mutate(claim.id);
                        }
                      } : undefined}
                    />
                  )
                : undefined
            }
            summaryTitle={(claim) => t('approvals.summaryTitle', { name: claim.employee_name, type: t('approvals.expenseClaim') })}
            renderSummary={(claim) => (
              <div className="space-y-5">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-11 h-11 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-900 dark:text-slate-100 font-semibold text-sm shrink-0">
                      {(claim.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div className="min-w-0">
                      <div className="font-semibold text-lg text-gray-900 dark:text-slate-100 break-words">{claim.employee_name}</div>
                      <div className="text-sm text-gray-500 dark:text-slate-400">{claim.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(claim.status)}
                </div>

                <div className="bg-gray-50 dark:bg-slate-800/70 rounded-lg p-4 space-y-4">
                  <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                    <div className="min-w-0">
                      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{t('approvals.claimTitle')}</p>
                      <p className="text-base font-semibold text-gray-900 dark:text-slate-100 mt-0.5 break-words">{claim.title}</p>
                    </div>
                    <div className="sm:text-right shrink-0">
                      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{t('common.amount')}</p>
                      <p className="text-xl font-bold text-gray-900 dark:text-slate-100 mt-0.5 tabular-nums">{fmt(claim.amount)}</p>
                    </div>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-3 border-t border-gray-200 dark:border-slate-700">
                    <SummaryField label={t('common.category')} value={claim.category || '\u2014'} />
                    <SummaryField label={t('approvals.expenseDate')} value={formatDate(claim.expense_date)} />
                  </div>
                </div>

                {claim.description && <SummaryField label={t('common.description')} value={claim.description} />}

                {claim.receipt_url && (
                  <div>
                    <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide mb-1">{t('approvals.receipt')}</p>
                    <AttachmentPreview url={claim.receipt_url} name={claim.receipt_file_name} />
                  </div>
                )}

                {claim.review_notes && (
                  <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/30 rounded-lg p-4">
                    <SummaryField label={t('approvals.reviewNotes')} value={claim.review_notes} />
                    {claim.reviewed_at && <p className="text-xs text-gray-500 dark:text-slate-400 mt-1">{t('approvals.reviewedOn', { date: formatDate(claim.reviewed_at) })}</p>}
                  </div>
                )}

                {canManageApprovals && claim.status === 'pending' && (
                  <div>
                    <label htmlFor={`review-claim-${claim.id}`} className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide block mb-1">
                      {t('approvals.reviewNotes')}
                    </label>
                    <textarea
                      id={`review-claim-${claim.id}`}
                      placeholder={t('approvals.notesPlaceholder')}
                      value={reviewNotes[claim.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [claim.id]: event.target.value }))}
                      rows={2}
                      className="form-input resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={
              canManageApprovals
                ? (claim, close) =>
                    claim.status === 'pending' ? (
                      <div className="flex flex-wrap items-center justify-end gap-3">
                        <button
                          type="button"
                          onClick={() => rejectClaimM.mutate({ id: claim.id, notes: reviewNotes[claim.id] }, { onSuccess: close })}
                          disabled={rejectClaimM.isPending || approveClaimM.isPending}
                          className="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/40 text-red-600 dark:text-red-400 text-sm font-medium rounded-lg hover:bg-red-50 dark:hover:bg-red-500/15 hover:border-red-300 disabled:opacity-50 transition-colors"
                        >
                          <XCircle className="w-4 h-4" />
                          {rejectClaimM.isPending ? t('common.rejecting') : t('common.reject')}
                        </button>
                        {isSelfApproval(claim.employee_id) ? (
                          <p className="text-sm text-gray-500 dark:text-slate-400 max-w-xs text-right">
                            {t('approvals.selfApprovalClaim')}
                          </p>
                        ) : (
                          <button
                            type="button"
                            onClick={() => approveClaimM.mutate({ id: claim.id, notes: reviewNotes[claim.id] }, { onSuccess: close })}
                            disabled={approveClaimM.isPending || rejectClaimM.isPending}
                            className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                          >
                            <CheckCircle className="w-4 h-4" />
                            {approveClaimM.isPending ? t('common.approving') : t('common.approve')}
                          </button>
                        )}
                      </div>
                    ) : null
                : undefined
            }
          />
        ) : (
          <DataTable
            columns={overtimeColumns}
            data={overtimeQuery.data?.data ?? []}
            total={overtimeQuery.data?.total}
            page={page}
            onPageChange={setPage}
            perPage={PER_PAGE}
            isLoading={overtimeQuery.isLoading}
            emptyMessage={t('approvals.noOvertime')}
            emptyIcon={<Clock className="w-8 h-8 text-gray-300 dark:text-slate-600" />}
            selectable={canManageApprovals}
            selectedRowKeys={selectedApprovalIds}
            onSelectedRowKeysChange={setSelectedApprovalIds}
            rowKey={(overtime) => overtime.id}
            renderActions={
              canManageApprovals
                ? (overtime) => (
                    <ActionButtons
                      onEdit={canEditOvertime(overtime) ? () => {
                        setOvertimeEditor(overtime);
                        setShowOvertimeModal(true);
                      } : undefined}
                      onCancel={canCancelOvertime(overtime) ? async () => {
                        const ok = await confirmDialog({
                          description: t('approvals.cancelOvertimeConfirm'),
                          confirmLabel: t('common.cancel'),
                          variant: 'warning',
                        });
                        if (ok) {
                          cancelOvertimeM.mutate(overtime.id);
                        }
                      } : undefined}
                      onDelete={canDeleteOvertime(overtime) ? async () => {
                        const ok = await confirmDialog({
                          description: t('approvals.deleteOvertimeConfirm'),
                          variant: 'danger',
                        });
                        if (ok) {
                          deleteOvertimeM.mutate(overtime.id);
                        }
                      } : undefined}
                    />
                  )
                : undefined
            }
            summaryTitle={(overtime) => t('approvals.summaryTitle', { name: overtime.employee_name, type: t('approvals.overtimeApplication') })}
            renderSummary={(overtime) => (
              <div className="space-y-5">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-11 h-11 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-900 dark:text-slate-100 font-semibold text-sm shrink-0">
                      {(overtime.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div className="min-w-0">
                      <div className="font-semibold text-lg text-gray-900 dark:text-slate-100 break-words">{overtime.employee_name}</div>
                      <div className="text-sm text-gray-500 dark:text-slate-400">{overtime.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(overtime.status)}
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 bg-gray-50 dark:bg-slate-800/70 rounded-lg p-4">
                  <SummaryField label={t('common.date')} value={formatDate(overtime.ot_date)} />
                  <SummaryField label={t('common.type')}>
                    <span
                      className={`text-sm font-medium mt-0.5 inline-block ${
                        overtime.ot_type === 'public_holiday'
                          ? 'text-red-700 dark:text-red-300'
                          : overtime.ot_type === 'rest_day'
                            ? 'text-blue-700 dark:text-blue-300'
                            : 'text-gray-900 dark:text-slate-100'
                      }`}
                    >
                      {otTypeLabel(overtime.ot_type)} ({otTypeMultiplier(overtime.ot_type)})
                    </span>
                  </SummaryField>
                  <SummaryField label={t('common.time')} value={`${overtime.start_time?.slice(0, 5)} \u2014 ${overtime.end_time?.slice(0, 5)}`} />
                  <SummaryField label={t('approvals.duration')} value={t('approvals.hoursCount', { count: Number(overtime.hours) })} />
                </div>

                {overtime.reason && <SummaryField label={t('common.reason')} value={overtime.reason} />}

                {overtime.review_notes && (
                  <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/30 rounded-lg p-4">
                    <SummaryField label={t('approvals.reviewNotes')} value={overtime.review_notes} />
                    {overtime.reviewed_at && <p className="text-xs text-gray-500 dark:text-slate-400 mt-1">{t('approvals.reviewedOn', { date: formatDate(overtime.reviewed_at) })}</p>}
                  </div>
                )}

                {canManageApprovals && overtime.status === 'pending' && (
                  <div>
                    <label htmlFor={`review-ot-${overtime.id}`} className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide block mb-1">
                      {t('approvals.reviewNotes')}
                    </label>
                    <textarea
                      id={`review-ot-${overtime.id}`}
                      placeholder={t('approvals.notesPlaceholder')}
                      value={reviewNotes[overtime.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [overtime.id]: event.target.value }))}
                      rows={2}
                      className="form-input resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={
              canManageApprovals
                ? (overtime, close) =>
                    overtime.status === 'pending' ? (
                      <div className="flex flex-wrap items-center justify-end gap-3">
                        <button
                          type="button"
                          onClick={() => rejectOvertimeM.mutate({ id: overtime.id, notes: reviewNotes[overtime.id] }, { onSuccess: close })}
                          disabled={rejectOvertimeM.isPending || approveOvertimeM.isPending}
                          className="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-slate-900 border border-red-200 dark:border-red-500/40 text-red-600 dark:text-red-400 text-sm font-medium rounded-lg hover:bg-red-50 dark:hover:bg-red-500/15 hover:border-red-300 disabled:opacity-50 transition-colors"
                        >
                          <XCircle className="w-4 h-4" />
                          {rejectOvertimeM.isPending ? t('common.rejecting') : t('common.reject')}
                        </button>
                        {isSelfApproval(overtime.employee_id) ? (
                          <p className="text-sm text-gray-500 dark:text-slate-400 max-w-xs text-right">
                            {t('approvals.selfApprovalOvertime')}
                          </p>
                        ) : (
                          <button
                            type="button"
                            onClick={() => approveOvertimeM.mutate({ id: overtime.id, notes: reviewNotes[overtime.id] }, { onSuccess: close })}
                            disabled={approveOvertimeM.isPending || rejectOvertimeM.isPending}
                            className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                          >
                            <CheckCircle className="w-4 h-4" />
                            {approveOvertimeM.isPending ? t('common.approving') : t('common.approve')}
                          </button>
                        )}
                      </div>
                    ) : null
                : undefined
            }
          />
        )}
      </div>

      <LeaveCrudModal
        open={showLeaveModal}
        onClose={() => {
          setShowLeaveModal(false);
          setLeaveEditor(null);
        }}
        item={leaveEditor}
        leaveTypes={leaveTypes}
        onSaved={() => {
          setShowLeaveModal(false);
          setLeaveEditor(null);
          refreshLeave();
          toast.success(t('approvals.savedSuccess'));
        }}
      />

      <ClaimCrudModal
        open={showClaimModal}
        onClose={() => {
          setShowClaimModal(false);
          setClaimEditor(null);
        }}
        item={claimEditor}
        onSaved={() => {
          setShowClaimModal(false);
          setClaimEditor(null);
          refreshClaims();
          toast.success(t('approvals.savedSuccess'));
        }}
      />

      <OvertimeCrudModal
        open={showOvertimeModal}
        onClose={() => {
          setShowOvertimeModal(false);
          setOvertimeEditor(null);
        }}
        item={overtimeEditor}
        onSaved={() => {
          setShowOvertimeModal(false);
          setOvertimeEditor(null);
          refreshOvertime();
          toast.success(t('approvals.savedSuccess'));
        }}
      />
    </>
  );
}
