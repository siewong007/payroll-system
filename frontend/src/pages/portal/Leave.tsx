import { useState } from 'react';
import { useTranslation, Trans } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Paperclip, ExternalLink, X, Calendar, AlertTriangle, Download, Trash2 } from 'lucide-react';
import { getLeaveBalances, getLeaveRequests, getLeaveTypes, createLeaveRequest, cancelLeaveRequest, deleteLeaveRequest, uploadFile, getMyProfile, exportLeaveIcs } from '@/api/portal';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';
import { formatDate, getErrorMessage } from '@/lib/utils';
import { AttachmentLink, AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { DataTable, type Column } from '@/components/ui/DataTable';
import type { Employee, LeaveBalance, LeaveRequest, LeaveType } from '@/types';
import i18n from '@/i18n';

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

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending', approved: 'badge-approved',
    rejected: 'badge-rejected', cancelled: 'badge-cancelled',
  };
  return (
    <span className={`badge ${cls[status] || 'badge-draft'}`}>
      {i18n.t(`enums.requestStatus.${status}`, { defaultValue: status })}
    </span>
  );
};

function getBalanceColumns(profile: Employee | undefined): Column<LeaveBalance>[] {
  return [
    { key: 'type', header: i18n.t('portal.leave.cols.type'), render: (b) => <span className="font-semibold text-gray-900">{b.leave_type_name}</span> },
    {
      key: 'status', header: i18n.t('portal.leave.cols.statusRemarks'), render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) {
          return (
            <span className="inline-flex items-center gap-1.5 text-amber-600 text-sm">
              <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
              {elig.reason}
            </span>
          );
        }
        const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
        return (
          <span className="text-gray-600 text-sm">
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
      key: 'entitled', header: i18n.t('portal.leave.cols.entitled'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? Number(b.entitled_days) : <span className="text-gray-300">{'\u2014'}</span>;
      },
    },
    {
      key: 'taken', header: i18n.t('portal.leave.cols.taken'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? Number(b.taken_days) : <span className="text-gray-300">{'\u2014'}</span>;
      },
    },
    {
      key: 'pending', header: i18n.t('portal.leave.cols.pending'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-300">{'\u2014'}</span>;
        return <span className={Number(b.pending_days) > 0 ? 'text-amber-600' : ''}>{Number(b.pending_days)}</span>;
      },
    },
    {
      key: 'balance', header: i18n.t('portal.leave.cols.balance'), align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-300">{'\u2014'}</span>;
        const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
        return <span className={`font-bold ${balance > 0 ? 'text-emerald-600' : balance < 0 ? 'text-red-600' : 'text-gray-600'}`}>{balance}</span>;
      },
    },
  ];
}

const getRequestColumns = (): Column<LeaveRequest>[] => [
  { key: 'type', header: i18n.t('portal.leave.cols.type'), render: (r) => <span className="font-semibold text-gray-900">{r.leave_type_name}</span> },
  {
    key: 'period', header: i18n.t('portal.leave.cols.period'), render: (r) => (
      <span className="text-gray-600">{formatDate(r.start_date)} – {formatDate(r.end_date)}</span>
    ),
  },
  { key: 'days', header: i18n.t('portal.leave.cols.days'), align: 'center', render: (r) => Number(r.days) },
  { key: 'reason', header: i18n.t('common.reason'), render: (r) => <span className="text-gray-400 max-w-[160px] truncate block">{r.reason || '\u2014'}</span> },
  {
    key: 'attachment', header: i18n.t('common.attachment'), render: (r) => (
      r.attachment_url ? (
        r.attachment_url.startsWith('blob:') ? (
          <span className="inline-flex items-center gap-1 text-red-400 text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[100px]">{i18n.t('common.unavailable')}</span>
          </span>
        ) : (
          <AttachmentLink url={r.attachment_url}
            className="inline-flex items-center gap-1 text-gray-900 hover:text-black text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[100px]">{r.attachment_name || i18n.t('common.view')}</span>
            <ExternalLink className="w-3 h-3" />
          </AttachmentLink>
        )
      ) : <span className="text-gray-300">\u2014</span>
    ),
  },
  { key: 'status', header: i18n.t('common.status'), align: 'center', render: (r) => statusBadge(r.status) },
];

const canCancelLeave = (request: LeaveRequest) => ['pending', 'approved', 'rejected'].includes(request.status);
const canDeleteLeave = (request: LeaveRequest) => request.status === 'cancelled';

export function Leave() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'balances' | 'requests'>('balances');
  const [showApply, setShowApply] = useState(false);
  const [selectedLeaveIds, setSelectedLeaveIds] = useState<string[]>([]);
  const [bulkError, setBulkError] = useState('');

  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  const currentYear = new Date().getFullYear();
  const { data: balances, isLoading: loadingBalances } = useQuery({
    queryKey: ['leave-balances', currentYear],
    queryFn: () => getLeaveBalances(currentYear),
  });

  const { data: requests } = useQuery({
    queryKey: ['leave-requests'],
    queryFn: getLeaveRequests,
  });

  const { data: leaveTypes } = useQuery({
    queryKey: ['leave-types'],
    queryFn: getLeaveTypes,
  });

  const cancelMutation = useMutation({
    mutationFn: cancelLeaveRequest,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
      queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteLeaveRequest,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['leave-requests'] });
      queryClient.invalidateQueries({ queryKey: ['leave-balances'] });
    },
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

  if (loadingBalances) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  if (showApply && leaveTypes) {
    return <ApplyLeaveForm leaveTypes={leaveTypes} onClose={() => setShowApply(false)} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">{t('portal.leave.title')}</h1>
          <p className="page-subtitle">{t('portal.leave.subtitle')}</p>
        </div>
        <div className="flex gap-2 w-full sm:w-auto">
          <button onClick={() => exportLeaveIcs()} className="btn-secondary w-full sm:w-auto">
            <Download className="w-4 h-4" /> {t('portal.leave.exportIcs')}
          </button>
          <button onClick={() => setShowApply(true)} className="btn-primary w-full sm:w-auto">
            <Plus className="w-4 h-4" /> {t('portal.leave.apply')}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200 overflow-x-auto">
        {[
          { key: 'balances' as const, label: t('portal.leave.tabs.balances') },
          { key: 'requests' as const, label: t('portal.leave.tabs.requests') },
        ].map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap transition-all-fast ${
              tab === t.key ? 'border-gray-900 text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'balances' ? (
        <DataTable
          columns={getBalanceColumns(profile)}
          data={balances ?? []}
          perPage={10}
          emptyMessage={t('portal.leave.emptyBalances')}
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
                <div className="col-span-2 pt-2 border-t border-gray-100">
                  <p className="text-xs text-gray-400 uppercase tracking-wide">{t('portal.leave.cols.balance')}</p>
                  <p className={`text-lg font-bold mt-0.5 ${balance > 0 ? 'text-emerald-600' : balance < 0 ? 'text-red-600' : 'text-gray-600'}`}>
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
              <div className="flex flex-col gap-2 rounded-xl border border-gray-200 bg-white px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                <span className="text-sm font-medium text-gray-700">{t('common.selected', { count: selectedLeaveIds.length })}</span>
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      if (confirm(t('portal.leave.bulkCancelConfirm', { count: selectedCancelableLeaveIds.length }))) {
                        setBulkError('');
                        bulkCancelMutation.mutate(selectedCancelableLeaveIds);
                      }
                    }}
                    disabled={selectedCancelableLeaveIds.length === 0 || bulkCancelMutation.isPending}
                    className="btn-secondary !py-2 text-sm disabled:opacity-50"
                  >
                    <X className="w-4 h-4" />
                    {bulkCancelMutation.isPending ? t('common.cancelling') : t('common.cancelSelected')}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      if (confirm(t('portal.leave.bulkDeleteConfirm', { count: selectedDeletableLeaveIds.length }))) {
                        setBulkError('');
                        bulkDeleteMutation.mutate(selectedDeletableLeaveIds);
                      }
                    }}
                    disabled={selectedDeletableLeaveIds.length === 0 || bulkDeleteMutation.isPending}
                    className="btn-secondary !py-2 text-sm text-red-600 hover:!bg-red-50 disabled:opacity-50"
                  >
                    <Trash2 className="w-4 h-4" />
                    {bulkDeleteMutation.isPending ? t('common.deleting') : t('common.deleteSelected')}
                  </button>
                </div>
              </div>
              {bulkError && (
                <div role="alert" className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
                  {bulkError}
                </div>
              )}
            </div>
          )}
          <DataTable
            columns={getRequestColumns()}
            data={requests ?? []}
            perPage={10}
            emptyMessage={t('portal.leave.emptyRequests')}
            emptyIcon={<Calendar className="w-10 h-10 opacity-40" />}
            selectable
            selectedRowKeys={selectedLeaveIds}
            onSelectedRowKeysChange={setSelectedLeaveIds}
            rowKey={(request) => request.id}
            summaryTitle={(r) => t('portal.leave.requestSummaryTitle', { type: r.leave_type_name })}
            renderSummary={(r) => (
            <div className="space-y-5">
              {/* Header */}
              <div className="flex items-center justify-between">
                <span className="font-semibold text-lg text-gray-900">{r.leave_type_name}</span>
                {statusBadge(r.status)}
              </div>

              {/* Details */}
              <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
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
                  <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">{t('common.attachment')}</p>
                  <AttachmentPreview url={r.attachment_url} name={r.attachment_name} />
                </div>
              )}

              {/* Review notes */}
              {r.review_notes && (
                <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                  <SummaryField label={t('portal.leave.fields.reviewNotes')} value={r.review_notes} />
                  {r.reviewed_at && (
                    <p className="text-xs text-gray-400 mt-1">{t('portal.leave.reviewedOn', { date: formatDate(r.reviewed_at) })}</p>
                  )}
                </div>
              )}
            </div>
            )}
            renderSummaryFooter={(r, close) =>
            canCancelLeave(r) || canDeleteLeave(r) ? (
              <div className="flex items-center justify-end gap-2">
                {canCancelLeave(r) && (
                  <button
                    onClick={() => {
                      if (confirm(t('portal.leave.cancelConfirm'))) {
                        cancelMutation.mutate(r.id, { onSuccess: close });
                      }
                    }}
                    disabled={cancelMutation.isPending}
                    className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                  >
                    <X className="w-4 h-4" />
                    {cancelMutation.isPending ? t('common.cancelling') : t('portal.leave.cancelRequest')}
                  </button>
                )}
                {canDeleteLeave(r) && (
                  <button
                    onClick={() => {
                      if (confirm(t('portal.leave.deleteConfirm'))) {
                        deleteMutation.mutate(r.id, { onSuccess: close });
                      }
                    }}
                    disabled={deleteMutation.isPending}
                    className="flex items-center gap-2 px-4 py-2.5 bg-red-600 text-white text-sm font-medium rounded-lg hover:bg-red-700 disabled:opacity-50 transition-colors"
                  >
                    <Trash2 className="w-4 h-4" />
                    {deleteMutation.isPending ? t('common.deleting') : t('portal.leave.deletePermanently')}
                  </button>
                )}
              </div>
            ) : null
            }
            renderActions={(r) => (
            <>
              {canCancelLeave(r) && (
                <button
                  onClick={() => {
                    if (confirm(t('portal.leave.cancelConfirm'))) cancelMutation.mutate(r.id);
                  }}
                  className="text-xs text-red-500 hover:text-red-700 font-medium"
                >
                  {t('common.cancel')}
                </button>
              )}
              {canDeleteLeave(r) && (
                <button
                  onClick={() => {
                    if (confirm(t('portal.leave.deleteConfirm'))) deleteMutation.mutate(r.id);
                  }}
                  className="text-xs text-red-500 hover:text-red-700 font-medium"
                >
                  {t('common.delete')}
                </button>
              )}
              {r.review_notes && (
                <div className="text-xs text-gray-400 mt-1">{r.review_notes}</div>
              )}
            </>
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
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 mt-0.5">{value}</p>
    </div>
  );
}

/* ───────────── Full-page Leave Application Form ───────────── */
function ApplyLeaveForm({ leaveTypes, onClose }: { leaveTypes: LeaveType[]; onClose: () => void }) {
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
  const [attachmentFile, setAttachmentFile] = useState<File | null>(null);
  const [attachmentPreview, setAttachmentPreview] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);

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

  const updateDays = (start: string, end: string) => {
    if (start && end) {
      const diff = Math.ceil((new Date(end).getTime() - new Date(start).getTime()) / 86400000) + 1;
      if (diff > 0) setForm((prev) => ({ ...prev, days: diff }));
    }
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setAttachmentFile(file);
      setAttachmentPreview(URL.createObjectURL(file));
      setForm((prev) => ({ ...prev, attachment_name: file.name }));
    }
  };

  const handleSubmit = () => {
    setError('');
    if (!form.leave_type_id) { setError(t('portal.leave.typeRequired')); return; }
    if (!form.start_date || !form.end_date) { setError(t('portal.leave.datesRequired')); return; }
    if (new Date(form.end_date) < new Date(form.start_date)) { setError(t('portal.leave.endAfterStart')); return; }
    if (form.days <= 0) { setError(t('portal.leave.daysPositive')); return; }
    mutation.mutate(form);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button onClick={onClose} className="btn-secondary !px-3 !py-2">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="page-header">
          <h1 className="page-title">{t('portal.leave.apply')}</h1>
          <p className="page-subtitle">{t('portal.leave.applySubtitle')}</p>
        </div>
      </div>

      {error && <div className="p-4 bg-red-50 text-red-700 text-sm rounded-xl border border-red-100">{error}</div>}
      {success && <div className="p-4 bg-emerald-50 text-emerald-700 text-sm rounded-xl border border-emerald-100">{t('portal.leave.submitted')}</div>}

      <div className="bg-white rounded-2xl shadow divide-y divide-gray-100">
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">{t('portal.leave.sections.type')}</span>
          </div>
          <div className="max-w-md">
            <label className="form-label">{t('portal.leave.selectType')} *</label>
            <select
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
        </div>

        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">{t('portal.leave.sections.period')}</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-2xl">
            <div>
              <label className="form-label">{t('portal.leave.fields.startDate')} *</label>
              <input
                type="date"
                value={form.start_date}
                onChange={(e) => {
                  setForm((prev) => ({ ...prev, start_date: e.target.value }));
                  updateDays(e.target.value, form.end_date);
                }}
                className="form-input"
              />
            </div>
            <div>
              <label className="form-label">{t('portal.leave.fields.endDate')} *</label>
              <input
                type="date"
                value={form.end_date}
                onChange={(e) => {
                  setForm((prev) => ({ ...prev, end_date: e.target.value }));
                  updateDays(form.start_date, e.target.value);
                }}
                className="form-input"
              />
            </div>
            <div>
              <label className="form-label">{t('portal.leave.fields.numDays')}</label>
              <input
                type="number"
                step="0.5"
                min="0.5"
                value={form.days}
                onChange={(e) => setForm((prev) => ({ ...prev, days: Number(e.target.value) }))}
                className="form-input"
              />
            </div>
          </div>
        </div>

        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">3</span>
            <span className="section-title">{t('portal.leave.sections.details')}</span>
          </div>
          <div className="space-y-5 max-w-2xl">
            <div>
              <label className="form-label">{t('common.reason')}</label>
              <textarea
                value={form.reason}
                onChange={(e) => setForm((prev) => ({ ...prev, reason: e.target.value }))}
                rows={3}
                className="form-input"
                placeholder={t('portal.leave.reasonPlaceholder')}
              />
            </div>
            <div>
              <label className="form-label">{t('common.attachment')}</label>
              <div className="flex items-center gap-3">
                <label className="flex items-center gap-2 px-4 py-2.5 bg-gray-50 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-100 cursor-pointer transition-all-fast">
                  <Paperclip className="w-4 h-4" />
                  <span>{form.attachment_name || t('common.chooseFile')}</span>
                  <input type="file" onChange={handleFileChange} accept=".pdf,.jpg,.jpeg,.png,.doc,.docx" className="hidden" />
                </label>
                {form.attachment_name && (
                  <button onClick={() => { setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' })); setAttachmentFile(null); if (attachmentPreview) { URL.revokeObjectURL(attachmentPreview); setAttachmentPreview(''); } }} className="text-gray-400 hover:text-red-500">
                    <X className="w-4 h-4" />
                  </button>
                )}
              </div>
              <p className="text-xs text-gray-400 mt-1.5">{t('portal.leave.attachmentHint')}</p>
            </div>
          </div>
        </div>
      </div>

      <div className="flex justify-end gap-3">
        <button onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
        <button onClick={handleSubmit} disabled={mutation.isPending || success} className="btn-primary">
          {mutation.isPending ? t('common.submitting') : success ? t('portal.leave.submittedShort') : t('portal.leave.submit')}
        </button>
      </div>
    </div>
  );
}
