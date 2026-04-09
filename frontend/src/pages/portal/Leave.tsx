import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Paperclip, ExternalLink, X, Calendar, FileText, AlertTriangle } from 'lucide-react';
import { getLeaveBalances, getLeaveRequests, getLeaveTypes, createLeaveRequest, cancelLeaveRequest, uploadFile, getMyProfile } from '@/api/portal';
import { formatDate } from '@/lib/utils';
import { DataTable, type Column } from '@/components/ui/DataTable';
import type { Employee, LeaveBalance, LeaveRequest, LeaveType } from '@/types';

function getLeaveEligibility(
  leaveType: string,
  profile: Employee | undefined,
): { eligible: boolean; reason?: string } {
  if (!profile) return { eligible: true };
  const name = leaveType.toLowerCase();

  if (name.includes('maternity')) {
    if (profile.gender !== 'female') return { eligible: false, reason: 'Available to female employees.' };
  }

  if (name.includes('paternity')) {
    if (profile.gender !== 'male') return { eligible: false, reason: 'Available to male employees.' };
    if ((profile.num_children ?? 0) === 0) return { eligible: false, reason: 'Employee has no registered children (requires date of birth).' };
  }

  if (name.includes('marriage')) {
    if (profile.marital_status === 'married') return { eligible: false, reason: 'Employee is already married.' };
  }

  return { eligible: true };
}

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending', approved: 'badge-approved',
    rejected: 'badge-rejected', cancelled: 'badge-cancelled',
  };
  return <span className={`badge ${cls[status] || 'badge-draft'}`}>{status}</span>;
};

function getBalanceColumns(profile: Employee | undefined): Column<LeaveBalance>[] {
  return [
    { key: 'type', header: 'Leave Type', render: (b) => <span className="font-semibold text-gray-900">{b.leave_type_name}</span> },
    {
      key: 'status', header: 'Status / Remarks', render: (b) => {
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
            Employee has <strong>{balance} day(s)</strong> of {b.leave_type_name}
          </span>
        );
      },
    },
    {
      key: 'entitled', header: 'Entitled', align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? Number(b.entitled_days) : <span className="text-gray-300">{'\u2014'}</span>;
      },
    },
    {
      key: 'taken', header: 'Taken', align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        return elig.eligible ? Number(b.taken_days) : <span className="text-gray-300">{'\u2014'}</span>;
      },
    },
    {
      key: 'pending', header: 'Pending', align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-300">{'\u2014'}</span>;
        return <span className={Number(b.pending_days) > 0 ? 'text-amber-600' : ''}>{Number(b.pending_days)}</span>;
      },
    },
    {
      key: 'balance', header: 'Balance', align: 'center', render: (b) => {
        const elig = getLeaveEligibility(b.leave_type_name, profile);
        if (!elig.eligible) return <span className="text-gray-300">{'\u2014'}</span>;
        const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
        return <span className={`font-bold ${balance > 0 ? 'text-emerald-600' : balance < 0 ? 'text-red-600' : 'text-gray-600'}`}>{balance}</span>;
      },
    },
  ];
}

const requestColumns: Column<LeaveRequest>[] = [
  { key: 'type', header: 'Type', render: (r) => <span className="font-semibold text-gray-900">{r.leave_type_name}</span> },
  {
    key: 'period', header: 'Period', render: (r) => (
      <span className="text-gray-600">{formatDate(r.start_date)} – {formatDate(r.end_date)}</span>
    ),
  },
  { key: 'days', header: 'Days', align: 'center', render: (r) => Number(r.days) },
  { key: 'reason', header: 'Reason', render: (r) => <span className="text-gray-400 max-w-[160px] truncate block">{r.reason || '\u2014'}</span> },
  {
    key: 'attachment', header: 'Attachment', render: (r) => (
      r.attachment_url ? (
        r.attachment_url.startsWith('blob:') ? (
          <span className="inline-flex items-center gap-1 text-red-400 text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[100px]">Unavailable</span>
          </span>
        ) : (
          <a href={r.attachment_url} target="_blank" rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-gray-900 hover:text-black text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[100px]">{r.attachment_name || 'View'}</span>
            <ExternalLink className="w-3 h-3" />
          </a>
        )
      ) : <span className="text-gray-300">\u2014</span>
    ),
  },
  { key: 'status', header: 'Status', align: 'center', render: (r) => statusBadge(r.status) },
];

export function Leave() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'balances' | 'requests'>('balances');
  const [showApply, setShowApply] = useState(false);

  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  const { data: balances, isLoading: loadingBalances } = useQuery({
    queryKey: ['leave-balances', 2026],
    queryFn: () => getLeaveBalances(2026),
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
          <h1 className="page-title">Leave</h1>
          <p className="page-subtitle">Manage your leave entitlements and requests</p>
        </div>
        <button onClick={() => setShowApply(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" /> Apply for Leave
        </button>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200 overflow-x-auto">
        {[
          { key: 'balances' as const, label: 'Leave Entitlement' },
          { key: 'requests' as const, label: 'Activities' },
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
          emptyMessage="No leave entitlements found"
          summaryTitle={(b) => `${b.leave_type_name} — Leave Balance`}
          renderSummary={(b) => {
            const balance = Number(b.entitled_days) + Number(b.carried_forward) - Number(b.taken_days) - Number(b.pending_days);
            return (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label="Leave Type" value={b.leave_type_name} />
                <SummaryField label="Year" value={String(b.year)} />
                <SummaryField label="Entitled" value={String(Number(b.entitled_days))} />
                <SummaryField label="Carried Forward" value={String(Number(b.carried_forward))} />
                <SummaryField label="Taken" value={String(Number(b.taken_days))} />
                <SummaryField label="Pending" value={String(Number(b.pending_days))} />
                <div className="col-span-2 pt-2 border-t border-gray-100">
                  <p className="text-xs text-gray-400 uppercase tracking-wide">Balance</p>
                  <p className={`text-lg font-bold mt-0.5 ${balance > 0 ? 'text-emerald-600' : balance < 0 ? 'text-red-600' : 'text-gray-600'}`}>
                    {balance} day(s)
                  </p>
                </div>
              </div>
            );
          }}
        />
      ) : (
        <DataTable
          columns={requestColumns}
          data={requests ?? []}
          perPage={10}
          emptyMessage="No leave requests yet"
          emptyIcon={<Calendar className="w-10 h-10 opacity-40" />}
          summaryTitle={(r) => `${r.leave_type_name} — Leave Request`}
          renderSummary={(r) => (
            <div className="space-y-5">
              {/* Header */}
              <div className="flex items-center justify-between">
                <span className="font-semibold text-lg text-gray-900">{r.leave_type_name}</span>
                {statusBadge(r.status)}
              </div>

              {/* Details */}
              <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
                <SummaryField label="Start Date" value={formatDate(r.start_date)} />
                <SummaryField label="End Date" value={formatDate(r.end_date)} />
                <SummaryField label="Duration" value={`${Number(r.days)} day(s)`} />
                <SummaryField label="Submitted" value={formatDate(r.created_at)} />
              </div>

              {/* Reason */}
              {r.reason && (
                <div>
                  <SummaryField label="Reason" value={r.reason} />
                </div>
              )}

              {/* Attachment preview */}
              {r.attachment_url && (
                <div>
                  <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">Attachment</p>
                  <AttachmentPreview url={r.attachment_url} name={r.attachment_name} />
                </div>
              )}

              {/* Review notes */}
              {r.review_notes && (
                <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                  <SummaryField label="Review Notes" value={r.review_notes} />
                  {r.reviewed_at && (
                    <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(r.reviewed_at)}</p>
                  )}
                </div>
              )}
            </div>
          )}
          renderSummaryFooter={(r, close) =>
            r.status === 'pending' ? (
              <div className="flex items-center justify-end">
                <button
                  onClick={() => {
                    cancelMutation.mutate(r.id, { onSuccess: close });
                  }}
                  disabled={cancelMutation.isPending}
                  className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                >
                  <X className="w-4 h-4" />
                  {cancelMutation.isPending ? 'Cancelling...' : 'Cancel Request'}
                </button>
              </div>
            ) : null
          }
          renderActions={(r) => (
            <>
              {r.status === 'pending' && (
                <button
                  onClick={() => cancelMutation.mutate(r.id)}
                  className="text-xs text-red-500 hover:text-red-700 font-medium"
                >
                  Cancel
                </button>
              )}
              {r.review_notes && (
                <div className="text-xs text-gray-400 mt-1">{r.review_notes}</div>
              )}
            </>
          )}
        />
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

function isImageUrl(url: string) {
  return /\.(jpg|jpeg|png|gif|webp|bmp|svg)(\?|$)/i.test(url);
}

function isBlobUrl(url: string) {
  return url.startsWith('blob:');
}

function AttachmentPreview({ url, name }: { url: string; name: string | null }) {
  const displayName = name || 'Attachment';

  if (isBlobUrl(url)) {
    return (
      <div className="mt-1 inline-flex items-center gap-2 px-4 py-3 bg-red-50 border border-red-200 rounded-lg">
        <div className="w-10 h-10 rounded-lg bg-red-100 flex items-center justify-center shrink-0">
          <FileText className="w-5 h-5 text-red-400" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium text-red-700 truncate">{displayName}</div>
          <div className="text-xs text-red-400">File unavailable — was not uploaded properly</div>
        </div>
      </div>
    );
  }

  const isImage = isImageUrl(url);

  return (
    <div className="mt-1">
      {isImage ? (
        <div className="space-y-2">
          <a href={url} target="_blank" rel="noopener noreferrer" className="block">
            <img
              src={url}
              alt={displayName}
              className="max-w-full max-h-64 rounded-lg border border-gray-200 object-contain bg-gray-50"
            />
          </a>
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 text-sm text-gray-900 hover:text-black"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            Open full size
          </a>
        </div>
      ) : (
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-4 py-3 bg-gray-50 border border-gray-200 rounded-lg hover:bg-gray-100 hover:border-gray-300 transition-colors group"
        >
          <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center shrink-0">
            <FileText className="w-5 h-5 text-gray-700" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-gray-700 group-hover:text-gray-900 truncate">{displayName}</div>
            <div className="text-xs text-gray-400">Click to open</div>
          </div>
          <ExternalLink className="w-4 h-4 text-gray-400 group-hover:text-gray-700 shrink-0" />
        </a>
      )}
    </div>
  );
}

/* ───────────── Full-page Leave Application Form ───────────── */
function ApplyLeaveForm({ leaveTypes, onClose }: { leaveTypes: LeaveType[]; onClose: () => void }) {
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
    onError: (err: any) => {
      setError(err.response?.data?.error || 'Failed to submit leave request. Please try again.');
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
    if (!form.leave_type_id) { setError('Please select a leave type'); return; }
    if (!form.start_date || !form.end_date) { setError('Please select start and end dates'); return; }
    if (new Date(form.end_date) < new Date(form.start_date)) { setError('End date must be after start date'); return; }
    if (form.days <= 0) { setError('Number of days must be greater than 0'); return; }
    mutation.mutate(form);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button onClick={onClose} className="btn-secondary !px-3 !py-2">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="page-header">
          <h1 className="page-title">Apply for Leave</h1>
          <p className="page-subtitle">Fill in the details below to submit your leave request</p>
        </div>
      </div>

      {error && <div className="p-4 bg-red-50 text-red-700 text-sm rounded-xl border border-red-100">{error}</div>}
      {success && <div className="p-4 bg-emerald-50 text-emerald-700 text-sm rounded-xl border border-emerald-100">Leave request submitted successfully!</div>}

      <div className="bg-white rounded-2xl shadow divide-y divide-gray-100">
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">Leave Type</span>
          </div>
          <div className="max-w-md">
            <label className="form-label">Select leave type *</label>
            <select
              value={form.leave_type_id}
              onChange={(e) => setForm((prev) => ({ ...prev, leave_type_id: e.target.value }))}
              className="form-input"
            >
              <option value="">Choose a leave type...</option>
              {leaveTypes.map((lt) => (
                <option key={lt.id} value={lt.id}>{lt.name}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">Leave Period</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-2xl">
            <div>
              <label className="form-label">Start Date *</label>
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
              <label className="form-label">End Date *</label>
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
              <label className="form-label">Number of Days</label>
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
            <span className="section-title">Additional Details</span>
          </div>
          <div className="space-y-5 max-w-2xl">
            <div>
              <label className="form-label">Reason</label>
              <textarea
                value={form.reason}
                onChange={(e) => setForm((prev) => ({ ...prev, reason: e.target.value }))}
                rows={3}
                className="form-input"
                placeholder="Optional reason for leave..."
              />
            </div>
            <div>
              <label className="form-label">Attachment</label>
              <div className="flex items-center gap-3">
                <label className="flex items-center gap-2 px-4 py-2.5 bg-gray-50 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-100 cursor-pointer transition-all-fast">
                  <Paperclip className="w-4 h-4" />
                  <span>{form.attachment_name || 'Choose file...'}</span>
                  <input type="file" onChange={handleFileChange} accept=".pdf,.jpg,.jpeg,.png,.doc,.docx" className="hidden" />
                </label>
                {form.attachment_name && (
                  <button onClick={() => { setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' })); setAttachmentFile(null); if (attachmentPreview) { URL.revokeObjectURL(attachmentPreview); setAttachmentPreview(''); } }} className="text-gray-400 hover:text-red-500">
                    <X className="w-4 h-4" />
                  </button>
                )}
              </div>
              <p className="text-xs text-gray-400 mt-1.5">PDF, JPG, PNG, DOC (optional supporting document)</p>
            </div>
          </div>
        </div>
      </div>

      <div className="flex justify-end gap-3">
        <button onClick={onClose} className="btn-secondary">Cancel</button>
        <button onClick={handleSubmit} disabled={mutation.isPending || success} className="btn-primary">
          {mutation.isPending ? 'Submitting...' : success ? 'Submitted!' : 'Submit Leave Request'}
        </button>
      </div>
    </div>
  );
}
