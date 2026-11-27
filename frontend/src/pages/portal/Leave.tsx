import { useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation, Trans } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Paperclip, ExternalLink, X, Calendar, AlertTriangle, Download, Trash2, Loader2 } from 'lucide-react';
import { getLeaveBalances, getLeaveRequests, getLeaveTypes, createLeaveRequest, cancelLeaveRequest, deleteLeaveRequest, uploadFile, getMyProfile, exportLeaveIcs, getPortalHolidays, getPortalWorkingDays } from '@/api/portal';
import { useAuth } from '@/context/AuthContext';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';
import { formatDate, getErrorMessage } from '@/lib/utils';
import { AttachmentLink, AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
import type { Employee, LeaveBalance, LeaveRequest, LeaveType } from '@/types';
import i18n from '@/i18n';

function parseLocalDate(dateStr: string): Date {
  const [y, m, d] = dateStr.slice(0, 10).split('-').map(Number);
  return new Date(y, m - 1, d);
}

function toIsoLocal(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function countWorkingDaysBetween(startStr: string, endStr: string, holidayDates: Set<string>): number {
  if (!startStr || !endStr) return 0;
  const start = parseLocalDate(startStr);
  const end = parseLocalDate(endStr);
  if (end < start) return 0;
  let count = 0;
  for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
    const dow = d.getDay();
    if (dow === 0 || dow === 6) continue;
    if (holidayDates.has(toIsoLocal(d))) continue;
    count += 1;
  }
  return count;
}

function getLeaveEligibility(
  leaveType: string,
  profile: Employee | undefined,
): { eligible: boolean; reason?: string } {
  if (!profile) return { eligible: true };
  const name = leaveType.toLowerCase();

  if (name.includes('maternity')) {
    if (profile.gender !== 'female') return { eligible: false, reason: i18n.t('portal.leave.eligibilityFemale') };
  }

  if (name.includes('paternity')) {
    if (profile.gender !== 'male') return { eligible: false, reason: i18n.t('portal.leave.eligibilityMale') };
    if ((profile.num_children ?? 0) === 0) return { eligible: false, reason: i18n.t('portal.leave.eligibilityChildren') };
  }

  if (name.includes('marriage')) {
    if (profile.marital_status === 'married') return { eligible: false, reason: i18n.t('portal.leave.eligibilityMarried') };
  }

  return { eligible: true };
}

const statusBadge = (status: string, t: TFunction) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending', approved: 'badge-approved',
    rejected: 'badge-rejected', cancelled: 'badge-cancelled',
  };
  return (
    <span className={`badge ${cls[status] || 'badge-draft'}`}>
      {t(`enums.requestStatus.${status}`, { defaultValue: status })}
    </span>
  );
};

function getBalanceColumns(
  profile: Employee | undefined,
  t: TFunction,
): Column<LeaveBalance>[] {
  return [
    { key: 'type', primary: true, header: t('portal.leave.cols.type'), render: (b) => <span className="font-semibold text-gray-900 dark:text-slate-100">{b.leave_type_name}</span> },
    {
      key: 'status', primary: true, header: t('portal.leave.cols.statusRemarks'), render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) {
          return (
            <span className="inline-flex items-center gap-1.5 text-amber-600 dark:text-amber-400 text-sm">
              <AlertTriangle className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
              {elig.reason}
            </span>
          );
        }
        const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
        return (
          <span className="text-gray-600 dark:text-slate-300 text-sm">
            <Trans
              i18nKey="portal.leave.hasDays"
              values={{ count: balance, type: b.leave_type_name }}
              components={{ strong: <strong /> }}
            />
          </span>
        );
      },
    },
    {
      key: 'entitled', header: t('portal.leave.cols.entitled'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? <span className="tabular-nums">{Number(b.entitled_days)}</span> : <span className="text-gray-400 dark:text-slate-500">—</span>;
      },
    },
    {
      key: 'taken', header: t('portal.leave.cols.taken'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? <span className="tabular-nums">{Number(b.taken_days)}</span> : <span className="text-gray-400 dark:text-slate-500">—</span>;
      },
    },
    {
      key: 'pending', header: t('portal.leave.cols.pending'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-400 dark:text-slate-500">—</span>;
        return <span className={`tabular-nums ${Number(b.pending_days) > 0 ? 'text-amber-600 dark:text-amber-400' : ''}`}>{Number(b.pending_days)}</span>;
      },
    },
    {
      key: 'balance', primary: true, header: t('portal.leave.cols.balance'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-400 dark:text-slate-500">—</span>;
        const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
        return <span className={`font-bold tabular-nums ${balance > 0 ? 'text-emerald-600 dark:text-emerald-400' : balance < 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-600 dark:text-slate-300'}`}>{balance}</span>;
      },
    },
  ];
}

const getRequestColumns = (
  t: TFunction,
): Column<LeaveRequest>[] => [
  { key: 'type', primary: true, header: t('portal.leave.cols.type'), render: (r) => <span className="font-semibold text-gray-900 dark:text-slate-100">{r.leave_type_name}</span> },
  {
    key: 'period', primary: true, header: t('portal.leave.cols.period'), render: (r) => (
      <span className="text-gray-600 dark:text-slate-300 tabular-nums whitespace-nowrap">{formatDate(r.start_date)} – {formatDate(r.end_date)}</span>
    ),
  },
  { key: 'days', primary: true, header: t('portal.leave.cols.days'), align: 'center', render: (r) => <span className="tabular-nums">{Number(r.days)}</span> },
  {
    key: 'reason', header: t('common.reason'), render: (r) => (
      <div className="min-w-0 max-w-[180px]">
        <span className="text-gray-500 dark:text-slate-400 truncate block">{r.reason || '—'}</span>
        {r.review_notes && (
          <p className="text-xs text-amber-600 dark:text-amber-400 truncate mt-0.5">{r.review_notes}</p>
        )}
      </div>
    ),
  },
  {
    key: 'attachment', header: t('common.attachment'), render: (r) => (
      r.attachment_url ? (
        r.attachment_url.startsWith('blob:') ? (
          <span className="inline-flex items-center gap-1 text-red-500 dark:text-red-400 text-sm">
            <Paperclip className="w-3 h-3" aria-hidden="true" />
            <span className="truncate max-w-[100px]">{t('common.unavailable')}</span>
          </span>
        ) : (
          <span onClick={(e) => e.stopPropagation()}>
            <AttachmentLink url={r.attachment_url}
              className="inline-flex items-center gap-1 text-gray-900 dark:text-slate-100 hover:underline text-sm">
              <Paperclip className="w-3 h-3" aria-hidden="true" />
              <span className="truncate max-w-[100px]">{r.attachment_name || t('common.view')}</span>
              <ExternalLink className="w-3 h-3" aria-hidden="true" />
            </AttachmentLink>
          </span>
        )
      ) : <span className="text-gray-400 dark:text-slate-500">—</span>
    ),
  },
  { key: 'status', primary: true, header: t('common.status'), align: 'center', render: (r) => statusBadge(r.status, t) },
];

const canCancelLeave = (request: LeaveRequest) => ['pending', 'approved', 'rejected'].includes(request.status);
const canDeleteLeave = (request: LeaveRequest) => request.status === 'cancelled';

export function Leave() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const hasEmployee = Boolean(user?.employee_id);
  const confirmDialog = useConfirm();
  const toast = useToast();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'balances' | 'requests'>('balances');
  const [showApply, setShowApply] = useState(false);
  const [selectedLeaveIds, setSelectedLeaveIds] = useState<string[]>([]);
  const [bulkError, setBulkError] = useState('');
  const [exportingIcs, setExportingIcs] = useState(false);
  const [exportError, setExportError] = useState('');

  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
    enabled: hasEmployee,
  });

  const currentYear = new Date().getFullYear();
  const {
    data: balances,
    isLoading: loadingBalances,
    isError: balancesError,
    error: balancesErr,
    refetch: refetchBalances,
  } = useQuery({
    queryKey: ['leave-balances', currentYear],
    queryFn: () => getLeaveBalances(currentYear),
    enabled: hasEmployee,
  });

  const {
    data: requests,
    isLoading: loadingRequests,
    isError: requestsError,
    error: requestsErr,
    refetch: refetchRequests,
  } = useQuery({
    queryKey: ['leave-requests'],
    queryFn: getLeaveRequests,
    enabled: hasEmployee,
  });

  const {
    data: leaveTypes,
    isLoading: loadingLeaveTypes,
    isError: leaveTypesError,
    refetch: refetchLeaveTypes,
  } = useQuery({
    queryKey: ['leave-types'],
    queryFn: getLeaveTypes,
    enabled: hasEmployee,
  });

  const cancelMutation = useMutation({
    mutationFn: cancelLeaveRequest,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
      queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('portal.leave.bulkCancelFailed'))),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteLeaveRequest,
    onSuccess: (_data, deletedId) => {
      setSelectedLeaveIds((prev) => prev.filter((id) => id !== deletedId));
      queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
      queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('portal.leave.bulkDeleteFailed'))),
  });

  const refreshLeave = () => {
    queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
    queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
  };

  // Partial failure is an outcome, not an exception: the refetch runs whatever
  // happens and only the requests that failed stay selected, so a retry hits
  // exactly those. `Promise.all` used to short-circuit on the first rejection,
  // leaving the balances stale and no error rendered anywhere.
  const bulkCancelMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, cancelLeaveRequest),
    onSuccess: (outcome) => {
      setSelectedLeaveIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'cancelled'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.leave.bulkCancelFailed'))),
    onSettled: refreshLeave,
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, deleteLeaveRequest),
    onSuccess: (outcome) => {
      setSelectedLeaveIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'deleted'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.leave.bulkDeleteFailed'))),
    onSettled: refreshLeave,
  });

  const selectedLeaveRequests = (requests ?? []).filter((request) => selectedLeaveIds.includes(request.id));
  const selectedCancelableLeaveIds = selectedLeaveRequests.filter(canCancelLeave).map((request) => request.id);
  const selectedDeletableLeaveIds = selectedLeaveRequests.filter(canDeleteLeave).map((request) => request.id);

  const balanceColumns = useMemo(() => getBalanceColumns(profile, t), [profile, t]);
  const requestColumns = useMemo(() => getRequestColumns(t), [t]);

  if (!hasEmployee) {
    return (
      <div className="space-y-6">
        <PageHeader className="mb-0" title={t('portal.leave.title')} subtitle={t('portal.leave.subtitle')} />
        <div className="card">
          <EmptyState icon={Calendar} title={t('attendance.scan.noEmployeeProfile')} />
        </div>
      </div>
    );
  }

  if (loadingBalances) {
    return (
      <div className="space-y-6" role="status" aria-busy="true" aria-label={t('common.loading')}>
        <PageHeader className="mb-0" title={t('portal.leave.title')} subtitle={t('portal.leave.subtitle')} />
        <Skeleton className="h-10 w-64 rounded-lg" />
        <div className="card space-y-3">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-12 w-full rounded-lg" />
          ))}
        </div>
      </div>
    );
  }

  if (balancesError || requestsError) {
    return (
      <div className="space-y-6">
        <PageHeader className="mb-0" title={t('portal.leave.title')} subtitle={t('portal.leave.subtitle')} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('portal.leave.loadFailed')}
            description={getErrorMessage(balancesErr || requestsErr, t('portal.leave.loadFailed'))}
            action={
              <button
                type="button"
                onClick={() => {
                  void refetchBalances();
                  void refetchRequests();
                }}
                className="btn-primary"
              >
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  if (showApply) {
    if (loadingLeaveTypes) {
      return (
        <div className="space-y-6" role="status" aria-busy="true" aria-label={t('common.loading')}>
          <div className="flex items-center gap-4">
            <button type="button" onClick={() => setShowApply(false)} aria-label={t('common.goBack')} className="btn-secondary min-h-11 min-w-11 !px-3 !py-2">
              <ArrowLeft className="w-4 h-4" aria-hidden="true" />
            </button>
            <PageHeader className="mb-0" title={t('portal.leave.apply')} subtitle={t('portal.leave.applySubtitle')} />
          </div>
          <div className="card p-6 space-y-4">
            <Skeleton className="h-6 w-48" />
            <Skeleton className="h-11 w-full max-w-md" />
            <Skeleton className="h-32 w-full" />
          </div>
        </div>
      );
    }
    if (leaveTypesError || !leaveTypes) {
      return (
        <div className="space-y-6">
          <div className="flex items-center gap-4">
            <button type="button" onClick={() => setShowApply(false)} aria-label={t('common.goBack')} className="btn-secondary min-h-11 min-w-11 !px-3 !py-2">
              <ArrowLeft className="w-4 h-4" aria-hidden="true" />
            </button>
            <PageHeader className="mb-0" title={t('portal.leave.apply')} subtitle={t('portal.leave.applySubtitle')} />
          </div>
          <div role="alert" className="card space-y-4 border-red-200 dark:border-red-500/30">
            <p className="text-sm text-red-700 dark:text-red-300">{t('portal.leave.typesLoadFailed')}</p>
            <div className="flex flex-wrap gap-2">
              <button type="button" onClick={() => void refetchLeaveTypes()} className="btn-primary">
                {t('common.retry')}
              </button>
              <button type="button" onClick={() => setShowApply(false)} className="btn-secondary">
                {t('common.goBack')}
              </button>
            </div>
          </div>
        </div>
      );
    }
    return (
      <ApplyLeaveForm
        leaveTypes={leaveTypes}
        balances={balances ?? []}
        profile={profile}
        onClose={() => setShowApply(false)}
      />
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        className="mb-0"
        title={t('portal.leave.title')}
        subtitle={t('portal.leave.subtitle')}
        actions={
          <div className="flex flex-col sm:flex-row gap-2 w-full sm:w-auto">
            <button
              type="button"
              disabled={exportingIcs}
              onClick={async () => {
                setExportError('');
                setExportingIcs(true);
                try {
                  await exportLeaveIcs();
                } catch (err) {
                  setExportError(getErrorMessage(err, t('portal.leave.exportFailed')));
                } finally {
                  setExportingIcs(false);
                }
              }}
              className="btn-secondary w-full sm:w-auto"
            >
              {exportingIcs ? (
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
              ) : (
                <Download className="w-4 h-4" aria-hidden="true" />
              )}
              {exportingIcs ? t('common.loading') : t('portal.leave.exportIcs')}
            </button>
            <button type="button" onClick={() => setShowApply(true)} className="btn-primary w-full sm:w-auto">
              <Plus className="w-4 h-4" aria-hidden="true" /> {t('portal.leave.apply')}
            </button>
          </div>
        }
      />

      {exportError && (
        <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
          {exportError}
        </div>
      )}

      {/* Tabs */}
      <div
        role="tablist"
        aria-label={t('portal.leave.title')}
        className="flex gap-1 border-b border-gray-200 dark:border-slate-800 overflow-x-auto scrollbar-thin"
      >
        {[
          { key: 'balances' as const, label: t('portal.leave.tabs.balances') },
          { key: 'requests' as const, label: t('portal.leave.tabs.requests') },
        ].map((item) => (
          <button
            key={item.key}
            role="tab"
            type="button"
            aria-selected={tab === item.key}
            onClick={() => setTab(item.key)}
            className={`min-h-11 sm:min-h-10 px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap transition-all-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              tab === item.key
                ? 'border-[var(--accent-1)] text-gray-900 dark:text-white'
                : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
            }`}
          >
            {item.label}
          </button>
        ))}
      </div>

      {tab === 'balances' ? (
        <DataTable
          columns={balanceColumns}
          data={balances ?? []}
          perPage={10}
          emptyMessage={t('portal.leave.emptyBalances')}
          emptyAction={
            <button type="button" onClick={() => setShowApply(true)} className="btn-primary">
              <Plus className="w-4 h-4" aria-hidden="true" /> {t('portal.leave.apply')}
            </button>
          }
          summaryTitle={(b) => t('portal.leave.balanceSummaryTitle', { type: b.leave_type_name })}
          renderSummary={(b) => {
            const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
            return (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label={t('portal.leave.cols.type')} value={b.leave_type_name} />
                <SummaryField label={t('common.year')} value={String(b.year)} />
                <SummaryField label={t('portal.leave.cols.entitled')} value={String(Number(b.entitled_days))} />
                <SummaryField label={t('portal.leave.cols.carriedForward')} value={String(Number(b.carried_forward))} />
                <SummaryField label={t('portal.leave.cols.taken')} value={String(Number(b.taken_days))} />
                <SummaryField label={t('portal.leave.cols.pending')} value={String(Number(b.pending_days))} />
                <div className="col-span-2 pt-2 border-t border-gray-100 dark:border-slate-800">
                  <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{t('portal.leave.cols.balance')}</p>
                  <p className={`text-lg font-bold tabular-nums mt-0.5 ${balance > 0 ? 'text-emerald-600 dark:text-emerald-400' : balance < 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-600 dark:text-slate-300'}`}>
                    {t('portal.leave.days', { count: balance })}
                  </p>
                </div>
              </div>
            );
          }}
        />
      ) : (
        <div className="space-y-3">
          {selectedLeaveIds.length > 0 && (
            <div className="space-y-2">
              <div className="flex flex-col gap-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                <span className="text-sm font-medium text-gray-700 dark:text-slate-200 tabular-nums">{t('common.selected', { count: selectedLeaveIds.length })}</span>
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={async () => {
                      const ok = await confirmDialog({
                        description: t('portal.leave.bulkCancelConfirm', { count: selectedCancelableLeaveIds.length }),
                        confirmLabel: t('common.cancelSelected'),
                        variant: 'warning',
                      });
                      if (ok) {
                        setBulkError('');
                        bulkCancelMutation.mutate(selectedCancelableLeaveIds);
                      }
                    }}
                    disabled={selectedCancelableLeaveIds.length === 0 || bulkCancelMutation.isPending}
                    className="btn-secondary min-h-11 sm:!py-2 text-sm disabled:opacity-50"
                  >
                    <X className="w-4 h-4" aria-hidden="true" />
                    {bulkCancelMutation.isPending ? t('common.cancelling') : t('common.cancelSelected')}
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      const ok = await confirmDialog({
                        description: t('portal.leave.bulkDeleteConfirm', { count: selectedDeletableLeaveIds.length }),
                        variant: 'danger',
                      });
                      if (ok) {
                        setBulkError('');
                        bulkDeleteMutation.mutate(selectedDeletableLeaveIds);
                      }
                    }}
                    disabled={selectedDeletableLeaveIds.length === 0 || bulkDeleteMutation.isPending}
                    className="btn-secondary min-h-11 sm:!py-2 text-sm text-red-600 dark:text-red-400 hover:!bg-red-50 dark:hover:!bg-red-500/10 disabled:opacity-50"
                  >
                    <Trash2 className="w-4 h-4" aria-hidden="true" />
                    {bulkDeleteMutation.isPending ? t('common.deleting') : t('common.deleteSelected')}
                  </button>
                </div>
              </div>
              {bulkError && (
                <div role="alert" className="rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300">
                  {bulkError}
                </div>
              )}
            </div>
          )}
          <DataTable
            columns={requestColumns}
            data={requests ?? []}
            isLoading={loadingRequests}
            perPage={10}
            emptyMessage={t('portal.leave.emptyRequests')}
            emptyIcon={<Calendar className="w-10 h-10 opacity-40" aria-hidden="true" />}
            emptyAction={
              <button type="button" onClick={() => setShowApply(true)} className="btn-primary">
                <Plus className="w-4 h-4" aria-hidden="true" /> {t('portal.leave.apply')}
              </button>
            }
            selectable
            selectedRowKeys={selectedLeaveIds}
            onSelectedRowKeysChange={setSelectedLeaveIds}
            rowKey={(request) => request.id}
            summaryTitle={(r) => t('portal.leave.requestSummaryTitle', { type: r.leave_type_name })}
            renderSummary={(r) => (
            <div className="space-y-5">
              {/* Header */}
              <div className="flex items-center justify-between gap-3">
                <span className="font-semibold text-lg text-gray-900 dark:text-slate-100">{r.leave_type_name}</span>
                {statusBadge(r.status, t)}
              </div>

              {/* Details */}
              <div className="grid grid-cols-2 gap-4 bg-gray-50 dark:bg-slate-800/70 rounded-lg p-4">
                <SummaryField label={t('portal.leave.fields.startDate')} value={formatDate(r.start_date)} />
                <SummaryField label={t('portal.leave.fields.endDate')} value={formatDate(r.end_date)} />
                <SummaryField label={t('portal.leave.fields.duration')} value={t('portal.leave.days', { count: Number(r.days) })} />
                <SummaryField label={t('portal.leave.fields.submitted')} value={formatDate(r.created_at)} />
              </div>

              {/* Reason */}
              {r.reason && (
                <div>
                  <SummaryField label={t('common.reason')} value={r.reason} />
                </div>
              )}

              {/* Attachment preview */}
              {r.attachment_url && (
                <div>
                  <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide mb-1">{t('common.attachment')}</p>
                  <AttachmentPreview url={r.attachment_url} name={r.attachment_name} />
                </div>
              )}

              {/* Review notes */}
              {r.review_notes && (
                <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/30 rounded-lg p-4">
                  <SummaryField label={t('portal.leave.fields.reviewNotes')} value={r.review_notes} />
                  {r.reviewed_at && (
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-1 tabular-nums">{t('portal.leave.reviewedOn', { date: formatDate(r.reviewed_at) })}</p>
                  )}
                </div>
              )}
            </div>
            )}
            renderSummaryFooter={(r, close) =>
            canCancelLeave(r) || canDeleteLeave(r) ? (
              <div className="flex flex-col-reverse sm:flex-row sm:items-center sm:justify-end gap-2">
                {canCancelLeave(r) && (
                  <button
                    type="button"
                    onClick={async () => {
                      const ok = await confirmDialog({
                        description: t('portal.leave.cancelConfirm'),
                        confirmLabel: t('portal.leave.cancelRequest'),
                        variant: 'warning',
                      });
                      if (ok) {
                        cancelMutation.mutate(r.id, { onSuccess: close });
                      }
                    }}
                    disabled={cancelMutation.isPending}
                    className="btn-secondary min-h-11 w-full sm:w-auto text-red-600 dark:text-red-400 hover:!bg-red-50 dark:hover:!bg-red-500/10"
                  >
                    <X className="w-4 h-4" aria-hidden="true" />
                    {cancelMutation.isPending ? t('common.cancelling') : t('portal.leave.cancelRequest')}
                  </button>
                )}
                {canDeleteLeave(r) && (
                  <button
                    type="button"
                    onClick={async () => {
                      const ok = await confirmDialog({
                        description: t('portal.leave.deleteConfirm'),
                        variant: 'danger',
                      });
                      if (ok) {
                        deleteMutation.mutate(r.id, { onSuccess: close });
                      }
                    }}
                    disabled={deleteMutation.isPending}
                    className="btn-danger min-h-11 w-full sm:w-auto"
                  >
                    <Trash2 className="w-4 h-4" aria-hidden="true" />
                    {deleteMutation.isPending ? t('common.deleting') : t('portal.leave.deletePermanently')}
                  </button>
                )}
              </div>
            ) : null
            }
            renderActions={(r) => (
            <div className="flex items-center justify-center gap-1">
              {canCancelLeave(r) && (
                <button
                  type="button"
                  disabled={cancelMutation.isPending || deleteMutation.isPending}
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.leave.cancelConfirm'),
                      confirmLabel: t('portal.leave.cancelRequest'),
                      variant: 'warning',
                    });
                    if (ok) cancelMutation.mutate(r.id);
                  }}
                  aria-label={`${t('portal.leave.cancelRequest')} — ${r.leave_type_name}`}
                  title={t('portal.leave.cancelRequest')}
                  className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  <X className="w-4 h-4" aria-hidden="true" />
                </button>
              )}
              {canDeleteLeave(r) && (
                <button
                  type="button"
                  disabled={cancelMutation.isPending || deleteMutation.isPending}
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.leave.deleteConfirm'),
                      variant: 'danger',
                    });
                    if (ok) deleteMutation.mutate(r.id);
                  }}
                  aria-label={t('common.deleteName', { name: r.leave_type_name })}
                  title={t('common.delete')}
                  className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-red-500 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-500/10 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  <Trash2 className="w-4 h-4" aria-hidden="true" />
                </button>
              )}
            </div>
            )}
          />
        </div>
      )}
    </div>
  );
}

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5 tabular-nums">{value}</p>
    </div>
  );
}

/* ───────────── Full-page Leave Application Form ───────────── */
function ApplyLeaveForm({
  leaveTypes,
  balances,
  profile,
  onClose,
}: {
  leaveTypes: LeaveType[];
  balances: LeaveBalance[];
  profile: Employee | undefined;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    leave_type_id: '',
    start_date: '',
    end_date: '',
    days: 1,
    reason: '',
    attachment_url: '',
    attachment_name: '',
  });
  const [halfDay, setHalfDay] = useState(false);
  const [daysManuallyEdited, setDaysManuallyEdited] = useState(false);
  const [attachmentFile, setAttachmentFile] = useState<File | null>(null);
  const [attachmentPreview, setAttachmentPreview] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const startYear = form.start_date ? parseLocalDate(form.start_date).getFullYear() : new Date().getFullYear();
  const endYear = form.end_date ? parseLocalDate(form.end_date).getFullYear() : startYear;

  const { data: holidaysStart = [] } = useQuery({
    queryKey: ['portal-holidays', startYear],
    queryFn: () => getPortalHolidays(startYear),
  });
  const { data: holidaysEnd = [] } = useQuery({
    queryKey: ['portal-holidays', endYear],
    queryFn: () => getPortalHolidays(endYear),
    enabled: endYear !== startYear,
  });

  const holidayDates = useMemo(() => {
    const dates = new Set<string>();
    for (const h of holidaysStart) dates.add(h.date.slice(0, 10));
    for (const h of holidaysEnd) dates.add(h.date.slice(0, 10));
    return dates;
  }, [holidaysStart, holidaysEnd]);

  const calculatedWorkingDays = useMemo(
    () => countWorkingDaysBetween(form.start_date, form.end_date, holidayDates),
    [form.start_date, form.end_date, holidayDates],
  );

  const validDateRange = Boolean(
    form.start_date &&
      form.end_date &&
      parseLocalDate(form.end_date) >= parseLocalDate(form.start_date),
  );

  const { data: serverWorkingDays } = useQuery({
    queryKey: ['portal-working-days', form.start_date, form.end_date],
    queryFn: () => getPortalWorkingDays(form.start_date, form.end_date),
    enabled: validDateRange,
  });

  const effectiveWorkingDays = serverWorkingDays?.working_days ?? calculatedWorkingDays;

  useEffect(() => {
    if (!validDateRange) return;
    if (effectiveWorkingDays !== 1) {
      setHalfDay(false);
    }
    if (!daysManuallyEdited) {
      setForm((prev) => ({
        ...prev,
        days: effectiveWorkingDays > 0 ? (effectiveWorkingDays === 1 && halfDay ? 0.5 : effectiveWorkingDays) : 0,
      }));
    }
  }, [validDateRange, effectiveWorkingDays, halfDay, daysManuallyEdited]);

  useEffect(() => {
    return () => {
      if (attachmentPreview) {
        URL.revokeObjectURL(attachmentPreview);
      }
    };
  }, [attachmentPreview]);

  const currentYear = new Date().getFullYear();
  const { data: yearBalances } = useQuery({
    queryKey: ['leave-balances', startYear],
    queryFn: () => getLeaveBalances(startYear),
    enabled: startYear !== currentYear,
  });
  const effectiveBalances = startYear !== currentYear && yearBalances ? yearBalances : balances;

  const selectedBalance = useMemo(
    () => effectiveBalances.find((b) => b.leave_type_id === form.leave_type_id),
    [effectiveBalances, form.leave_type_id],
  );
  const selectedType = useMemo(
    () => leaveTypes.find((lt) => lt.id === form.leave_type_id),
    [leaveTypes, form.leave_type_id],
  );
  const eligibility = useMemo(
    () => (selectedType ? getLeaveEligibility(selectedType.name, profile) : { eligible: true }),
    [selectedType, profile],
  );
  const availableDays = selectedBalance
    ? Number(selectedBalance.entitled_days) + Number(selectedBalance.carried_forward) - Number(selectedBalance.taken_days) - Number(selectedBalance.pending_days)
    : null;

  const mutation = useMutation({
    mutationFn: async (data: typeof form) => {
      let attachmentUrl = data.attachment_url || undefined;
      let attachmentName = data.attachment_name || undefined;

      if (attachmentFile) {
        const uploaded = await uploadFile(attachmentFile);
        attachmentUrl = uploaded.url;
        attachmentName = uploaded.file_name;
      }

      return createLeaveRequest({
        leave_type_id: data.leave_type_id,
        start_date: data.start_date,
        end_date: data.end_date,
        days: data.days,
        reason: data.reason || undefined,
        attachment_url: attachmentUrl,
        attachment_name: attachmentName,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
      queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
      setSuccess(true);
      setTimeout(() => onClose(), 1200);
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('portal.leave.submitFailed')));
    },
  });

  const syncDatesAndDays = (start: string, end: string) => {
    setDaysManuallyEdited(false);
    const working = countWorkingDaysBetween(start, end, holidayDates);
    if (working !== 1) {
      setHalfDay(false);
    }
    setForm((prev) => ({
      ...prev,
      start_date: start,
      end_date: end,
      days: working > 0 ? (working === 1 && halfDay ? 0.5 : working) : 0,
    }));
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.currentTarget.value = '';
    if (file) {
      if (attachmentPreview) URL.revokeObjectURL(attachmentPreview);
      setAttachmentFile(file);
      setAttachmentPreview(file.type.startsWith('image/') ? URL.createObjectURL(file) : '');
      setForm((prev) => ({ ...prev, attachment_name: file.name }));
    }
  };

  const handleSubmit = () => {
    setError('');
    if (!form.leave_type_id) { setError(t('portal.leave.typeRequired')); return; }
    if (!form.start_date || !form.end_date) { setError(t('portal.leave.datesRequired')); return; }
    if (parseLocalDate(form.end_date) < parseLocalDate(form.start_date)) { setError(t('portal.leave.endAfterStart')); return; }
    if (effectiveWorkingDays === 0 && !daysManuallyEdited) { setError(t('portal.leave.noWorkingDays')); return; }
    if (form.days <= 0) { setError(t('portal.leave.daysPositive')); return; }
    mutation.mutate(form);
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        handleSubmit();
      }}
      className="space-y-6"
    >
      <div className="flex items-center gap-4">
        <button type="button" onClick={onClose} aria-label={t('common.goBack')} className="btn-secondary min-h-11 min-w-11 !px-3 !py-2 shrink-0">
          <ArrowLeft className="w-4 h-4" aria-hidden="true" />
        </button>
        <PageHeader className="mb-0" title={t('portal.leave.apply')} subtitle={t('portal.leave.applySubtitle')} />
      </div>

      {error && (
        <div role="alert" className="p-4 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/30">
          {error}
        </div>
      )}
      {success && (
        <div role="status" className="p-4 bg-emerald-50 dark:bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 text-sm rounded-xl border border-emerald-100 dark:border-emerald-500/30">
          {t('portal.leave.submitted')}
        </div>
      )}

      <div className="card !p-0 divide-y divide-gray-100 dark:divide-slate-800">
        <div className="p-5 sm:p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">{t('portal.leave.sections.type')}</span>
          </div>
          <div className="max-w-md space-y-3">
            <div>
              <label className="form-label" htmlFor="leave-type">{t('portal.leave.selectType')} *</label>
              <select
                id="leave-type"
                required
                value={form.leave_type_id}
                onChange={(e) => setForm((prev) => ({ ...prev, leave_type_id: e.target.value }))}
                className="form-input"
              >
                <option value="">{t('portal.leave.chooseType')}</option>
                {leaveTypes.map((lt) => (
                  <option key={lt.id} value={lt.id}>{lt.name}</option>
                ))}
              </select>
            </div>
            {selectedType && !eligibility.eligible && (
              <div role="status" className="flex items-start gap-2 rounded-xl bg-amber-50 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/30 p-3 text-xs text-amber-800 dark:text-amber-300">
                <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" aria-hidden="true" />
                <span>{eligibility.reason}</span>
              </div>
            )}
            {selectedType && eligibility.eligible && availableDays !== null && (
              <p className="text-xs font-medium text-[var(--accent-1)] tabular-nums">
                {t('portal.leave.availableBalance', { count: availableDays })}
              </p>
            )}
          </div>
        </div>

        <div className="p-5 sm:p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">{t('portal.leave.sections.period')}</span>
          </div>
          <div className="space-y-4 max-w-2xl">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div>
                <label className="form-label" htmlFor="leave-start">{t('portal.leave.fields.startDate')} *</label>
                <input
                  id="leave-start"
                  type="date"
                  required
                  value={form.start_date}
                  onChange={(e) => syncDatesAndDays(e.target.value, form.end_date)}
                  className="form-input tabular-nums"
                />
              </div>
              <div>
                <label className="form-label" htmlFor="leave-end">{t('portal.leave.fields.endDate')} *</label>
                <input
                  id="leave-end"
                  type="date"
                  required
                  value={form.end_date}
                  onChange={(e) => syncDatesAndDays(form.start_date, e.target.value)}
                  className="form-input tabular-nums"
                />
              </div>
              <div>
                <label className="form-label" htmlFor="leave-days">{t('portal.leave.fields.numDays')}</label>
                <input
                  id="leave-days"
                  type="number"
                  step="0.5"
                  min="0.5"
                  value={form.days}
                  onChange={(e) => {
                    const v = Number(e.target.value);
                    setDaysManuallyEdited(true);
                    setHalfDay(v === 0.5 && effectiveWorkingDays === 1);
                    setForm((prev) => ({ ...prev, days: v }));
                  }}
                  className="form-input tabular-nums"
                />
              </div>
            </div>
            {validDateRange && (
              <div className="flex flex-wrap items-center justify-between gap-3 text-xs text-gray-500 dark:text-slate-400">
                <span className="tabular-nums">{t('portal.leave.workingDaysHint', { count: effectiveWorkingDays })}</span>
                {effectiveWorkingDays === 1 && (
                  <label className="min-h-11 sm:min-h-9 inline-flex items-center gap-2 font-medium text-gray-700 dark:text-slate-200 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={halfDay}
                      onChange={(e) => {
                        const checked = e.target.checked;
                        setHalfDay(checked);
                        setForm((prev) => ({ ...prev, days: checked ? 0.5 : 1 }));
                      }}
                      className="h-4 w-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)] focus:ring-[var(--ring)]"
                    />
                    {t('portal.leave.halfDay')}
                  </label>
                )}
              </div>
            )}
          </div>
        </div>

        <div className="p-5 sm:p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">3</span>
            <span className="section-title">{t('portal.leave.sections.details')}</span>
          </div>
          <div className="space-y-5 max-w-2xl">
            <div>
              <label className="form-label" htmlFor="leave-reason">{t('common.reason')}</label>
              <textarea
                id="leave-reason"
                value={form.reason}
                onChange={(e) => setForm((prev) => ({ ...prev, reason: e.target.value }))}
                rows={3}
                className="form-input"
                placeholder={t('portal.leave.reasonPlaceholder')}
              />
            </div>
            <div>
              <span className="form-label">{t('common.attachment')}</span>
              <div className="flex items-center gap-3">
                <label className="min-h-11 sm:min-h-10 flex items-center gap-2 px-4 py-2.5 bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg text-sm text-gray-600 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-700 cursor-pointer transition-all-fast focus-within:ring-2 focus-within:ring-[var(--ring)]">
                  <Paperclip className="w-4 h-4 shrink-0" aria-hidden="true" />
                  <span className="truncate max-w-[220px]">{form.attachment_name || t('common.chooseFile')}</span>
                  <input ref={fileInputRef} id="leave-attachment" type="file" onChange={handleFileChange} accept=".pdf,.jpg,.jpeg,.png,.doc,.docx" className="sr-only" />
                </label>
                {form.attachment_name && (
                  <button
                    type="button"
                    aria-label={t('common.removeFile')}
                    onClick={() => {
                      if (fileInputRef.current) fileInputRef.current.value = '';
                      setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' }));
                      setAttachmentFile(null);
                      if (attachmentPreview) {
                        URL.revokeObjectURL(attachmentPreview);
                        setAttachmentPreview('');
                      }
                    }}
                    className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                  >
                    <X className="w-4 h-4" aria-hidden="true" />
                  </button>
                )}
              </div>
              {attachmentPreview && (
                <img
                  src={attachmentPreview}
                  alt={form.attachment_name}
                  className="mt-3 h-24 w-24 object-cover rounded-lg border border-gray-200 dark:border-slate-700"
                />
              )}
              <p className="text-xs text-gray-500 dark:text-slate-400 mt-1.5">{t('portal.leave.attachmentHint')}</p>
            </div>
          </div>
        </div>
      </div>

      <div className="flex flex-col-reverse sm:flex-row sm:justify-end gap-3">
        <button type="button" onClick={onClose} disabled={mutation.isPending} className="btn-secondary">{t('common.cancel')}</button>
        <button type="submit" disabled={mutation.isPending || success} className="btn-primary">
          {mutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
          {mutation.isPending ? t('common.submitting') : success ? t('portal.leave.submittedShort') : t('portal.leave.submit')}
        </button>
      </div>
    </form>
  );
}
