import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { CheckCircle, XCircle, Clock, Filter, Paperclip, ExternalLink, FileText, Image } from 'lucide-react';
import {
  getLeaveRequests,
  approveLeave,
  rejectLeave,
  getClaims,
  approveClaim,
  rejectClaim,
  getOvertimeRequests,
  approveOvertime,
  rejectOvertime,
  type LeaveRequestWithEmployee,
  type ClaimWithEmployee,
  type OvertimeWithEmployee,
} from '@/api/approvals';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { formatDate } from '@/lib/utils';

const fmt = (sen: number) => `RM ${(sen / 100).toFixed(2)}`;

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending', approved: 'badge-approved', rejected: 'badge-rejected',
    cancelled: 'badge-cancelled', draft: 'badge-draft', processed: 'badge-processed',
  };
  return <span className={`badge ${cls[status] || 'badge-draft'}`}>{status}</span>;
};

function isImageUrl(url: string) {
  return /\.(jpg|jpeg|png|gif|webp|bmp|svg)(\?|$)/i.test(url);
}

function AttachmentPreview({ url, name }: { url: string; name: string | null }) {
  const displayName = name || 'Attachment';

  if (url.startsWith('blob:')) {
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

function SummaryField({ label, value, children }: { label: string; value?: string; children?: React.ReactNode }) {
  return (
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      {children ?? <p className="text-sm font-medium text-gray-900 mt-0.5">{value || '\u2014'}</p>}
    </div>
  );
}

export function Approvals() {
  const [tab, setTab] = useState<'leave' | 'claims' | 'overtime'>('leave');
  const [statusFilter, setStatusFilter] = useState<string>('pending');
  const [reviewNotes, setReviewNotes] = useState<Record<string, string>>({});
  const queryClient = useQueryClient();

  const leaveQuery = useQuery({
    queryKey: ['approvals-leave', statusFilter],
    queryFn: () => getLeaveRequests(statusFilter || undefined),
    enabled: tab === 'leave',
  });

  const claimsQuery = useQuery({
    queryKey: ['approvals-claims', statusFilter],
    queryFn: () => getClaims(statusFilter || undefined),
    enabled: tab === 'claims',
  });

  const approveLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveLeave(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-leave'] }),
  });

  const rejectLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectLeave(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-leave'] }),
  });

  const approveClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveClaim(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-claims'] }),
  });

  const rejectClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectClaim(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-claims'] }),
  });

  const overtimeQuery = useQuery({
    queryKey: ['approvals-overtime', statusFilter],
    queryFn: () => getOvertimeRequests(statusFilter || undefined),
    enabled: tab === 'overtime',
  });

  const approveOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveOvertime(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-overtime'] }),
  });

  const rejectOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectOvertime(id, notes),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals-overtime'] }),
  });

  const otTypeLabel = (t: string) => {
    const labels: Record<string, string> = { normal: 'Normal Day', rest_day: 'Rest Day', public_holiday: 'Public Holiday' };
    return labels[t] || t;
  };

  const otTypeMultiplier = (t: string) => {
    const m: Record<string, string> = { normal: '1.5x', rest_day: '2.0x', public_holiday: '3.0x' };
    return m[t] || '';
  };

  const overtimeColumns: Column<OvertimeWithEmployee>[] = [
    {
      key: 'employee', header: 'Employee', render: (o) => (
        <div>
          <div className="font-semibold text-gray-900">{o.employee_name}</div>
          <div className="text-xs text-gray-400">{o.employee_number}</div>
        </div>
      ),
    },
    { key: 'date', header: 'Date', render: (o) => <span className="text-gray-700">{o.ot_date}</span> },
    {
      key: 'type', header: 'Type', render: (o) => (
        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
          o.ot_type === 'public_holiday' ? 'bg-red-100 text-red-700' :
          o.ot_type === 'rest_day' ? 'bg-blue-100 text-blue-700' :
          'bg-gray-100 text-gray-600'
        }`}>
          {otTypeLabel(o.ot_type)} ({otTypeMultiplier(o.ot_type)})
        </span>
      ),
    },
    { key: 'time', header: 'Time', render: (o) => <span className="text-gray-500">{o.start_time?.slice(0, 5)} — {o.end_time?.slice(0, 5)}</span> },
    { key: 'hours', header: 'Hours', align: 'right', render: (o) => <span className="font-semibold">{o.hours}h</span> },
    { key: 'reason', header: 'Reason', render: (o) => <span className="text-gray-400 max-w-[180px] truncate block">{o.reason || '\u2014'}</span> },
    { key: 'status', header: 'Status', render: (o) => statusBadge(o.status) },
  ];

  const leaveColumns: Column<LeaveRequestWithEmployee>[] = [
    {
      key: 'employee', header: 'Employee', render: (lr) => (
        <div>
          <div className="font-semibold text-gray-900">{lr.employee_name}</div>
          <div className="text-xs text-gray-400">{lr.employee_number}</div>
        </div>
      ),
    },
    { key: 'type', header: 'Leave Type', render: (lr) => lr.leave_type_name },
    { key: 'period', header: 'Period', render: (lr) => <span className="text-gray-500">{lr.start_date} &rarr; {lr.end_date}</span> },
    { key: 'days', header: 'Days', render: (lr) => lr.days },
    { key: 'reason', header: 'Reason', render: (lr) => <span className="text-gray-400 max-w-[180px] truncate block">{lr.reason || '\u2014'}</span> },
    {
      key: 'attachment', header: 'Attachment', render: (lr) => (
        lr.attachment_url ? (
          <span className="inline-flex items-center gap-1 text-gray-900 text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[80px]">{lr.attachment_name || 'File'}</span>
          </span>
        ) : <span className="text-gray-300">\u2014</span>
      ),
    },
    { key: 'status', header: 'Status', render: (lr) => statusBadge(lr.status) },
  ];

  const claimColumns: Column<ClaimWithEmployee>[] = [
    {
      key: 'employee', header: 'Employee', render: (c) => (
        <div>
          <div className="font-semibold text-gray-900">{c.employee_name}</div>
          <div className="text-xs text-gray-400">{c.employee_number}</div>
        </div>
      ),
    },
    {
      key: 'title', header: 'Title', render: (c) => (
        <div>
          <div className="font-medium">{c.title}</div>
          {c.description && <div className="text-xs text-gray-400 truncate max-w-[180px]">{c.description}</div>}
        </div>
      ),
    },
    { key: 'category', header: 'Category', render: (c) => <span className="text-gray-500">{c.category || '\u2014'}</span> },
    { key: 'amount', header: 'Amount', align: 'right', render: (c) => <span className="font-semibold">{fmt(c.amount)}</span> },
    { key: 'date', header: 'Date', render: (c) => <span className="text-gray-500">{c.expense_date}</span> },
    {
      key: 'receipt', header: 'Receipt', render: (c) => (
        c.receipt_url ? (
          c.receipt_url.startsWith('blob:') ? (
            <span className="inline-flex items-center gap-1 text-red-400 text-sm">
              <Paperclip className="w-3 h-3" />
              <span className="truncate max-w-[80px]">Unavailable</span>
            </span>
          ) : (
            <a href={c.receipt_url} target="_blank" rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-gray-900 hover:text-black text-sm">
              {isImageUrl(c.receipt_url) ? <Image className="w-3 h-3" /> : <Paperclip className="w-3 h-3" />}
              <span className="truncate max-w-[80px]">{c.receipt_file_name || 'Receipt'}</span>
              <ExternalLink className="w-3 h-3" />
            </a>
          )
        ) : <span className="text-gray-300">\u2014</span>
      ),
    },
    { key: 'status', header: 'Status', render: (c) => statusBadge(c.status) },
  ];

  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">Approvals</h1>
        <p className="page-subtitle">Review and manage leave requests and expense claims</p>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200">
        {(['leave', 'claims', 'overtime'] as const).map((t) => (
          <button
            key={t}
            onClick={() => { setTab(t); setStatusFilter('pending'); }}
            className={`px-5 py-2.5 text-sm font-medium border-b-2 transition-all-fast ${
              tab === t ? 'border-black text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
            }`}
          >
            {t === 'leave' ? 'Leave Requests' : t === 'claims' ? 'Expense Claims' : 'Overtime'}
          </button>
        ))}
      </div>

      {/* Status filter */}
      <div className="flex items-center gap-2">
        <Filter className="w-4 h-4 text-gray-300" />
        {['pending', 'approved', 'rejected', ''].map((s) => (
          <button
            key={s}
            onClick={() => setStatusFilter(s)}
            className={`px-3.5 py-1.5 text-xs font-medium rounded-full transition-all-fast ${
              statusFilter === s
                ? 'bg-black text-white shadow-sm'
                : 'bg-white text-gray-500 border border-gray-200 hover:border-gray-300'
            }`}
          >
            {s || 'All'}
          </button>
        ))}
      </div>

      {tab === 'leave' && (
        <DataTable
          columns={leaveColumns}
          data={leaveQuery.data ?? []}
          perPage={10}
          isLoading={leaveQuery.isLoading}
          emptyMessage="No leave requests found"
          emptyIcon={<Clock className="w-8 h-8 text-gray-200" />}
          summaryTitle={(lr) => `${lr.employee_name} — Leave Request`}
          renderSummary={(lr) => (
            <div className="space-y-5">
              {/* Header */}
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                    {(lr.employee_name || '').split(' ').map(n => n[0]).slice(0, 2).join('')}
                  </div>
                  <div>
                    <div className="font-semibold text-lg text-gray-900">{lr.employee_name}</div>
                    <div className="text-sm text-gray-400">{lr.employee_number}</div>
                  </div>
                </div>
                {statusBadge(lr.status)}
              </div>

              {/* Details */}
              <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
                <SummaryField label="Leave Type" value={lr.leave_type_name || '\u2014'} />
                <SummaryField label="Duration" value={`${lr.days} day(s)`} />
                <SummaryField label="Start Date" value={formatDate(lr.start_date)} />
                <SummaryField label="End Date" value={formatDate(lr.end_date)} />
              </div>

              {/* Reason */}
              {lr.reason && (
                <div>
                  <SummaryField label="Reason" value={lr.reason} />
                </div>
              )}

              {/* Attachment */}
              {lr.attachment_url && (
                <div>
                  <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">Attachment</p>
                  <AttachmentPreview url={lr.attachment_url} name={lr.attachment_name} />
                </div>
              )}

              {/* Review Notes (if already reviewed) */}
              {lr.review_notes && (
                <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                  <SummaryField label="Review Notes" value={lr.review_notes} />
                  {lr.reviewed_at && (
                    <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(lr.reviewed_at)}</p>
                  )}
                </div>
              )}

              {/* Notes input for pending items */}
              {lr.status === 'pending' && (
                <div>
                  <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                  <textarea
                    placeholder="Add notes for your decision (optional)..."
                    value={reviewNotes[lr.id] || ''}
                    onChange={(e) => setReviewNotes((prev) => ({ ...prev, [lr.id]: e.target.value }))}
                    rows={2}
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                  />
                </div>
              )}
            </div>
          )}
          renderSummaryFooter={(lr, close) =>
            lr.status === 'pending' ? (
              <div className="flex items-center justify-end gap-3">
                <button
                  onClick={() => {
                    rejectLeaveM.mutate({ id: lr.id, notes: reviewNotes[lr.id] }, { onSuccess: close });
                  }}
                  disabled={rejectLeaveM.isPending}
                  className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                >
                  <XCircle className="w-4 h-4" />
                  {rejectLeaveM.isPending ? 'Rejecting...' : 'Reject'}
                </button>
                <button
                  onClick={() => {
                    approveLeaveM.mutate({ id: lr.id, notes: reviewNotes[lr.id] }, { onSuccess: close });
                  }}
                  disabled={approveLeaveM.isPending}
                  className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                >
                  <CheckCircle className="w-4 h-4" />
                  {approveLeaveM.isPending ? 'Approving...' : 'Approve'}
                </button>
              </div>
            ) : null
          }
        />
      )}

      {tab === 'claims' && (
        <DataTable
          columns={claimColumns}
          data={claimsQuery.data ?? []}
          perPage={10}
          isLoading={claimsQuery.isLoading}
          emptyMessage="No claims found"
          emptyIcon={<Clock className="w-8 h-8 text-gray-200" />}
          summaryTitle={(c) => `${c.employee_name} — Expense Claim`}
          renderSummary={(c) => (
            <div className="space-y-5">
              {/* Header */}
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                    {(c.employee_name || '').split(' ').map(n => n[0]).slice(0, 2).join('')}
                  </div>
                  <div>
                    <div className="font-semibold text-lg text-gray-900">{c.employee_name}</div>
                    <div className="text-sm text-gray-400">{c.employee_number}</div>
                  </div>
                </div>
                {statusBadge(c.status)}
              </div>

              {/* Claim details */}
              <div className="bg-gray-50 rounded-lg p-4 space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-gray-400 uppercase tracking-wide">Claim Title</p>
                    <p className="text-base font-semibold text-gray-900 mt-0.5">{c.title}</p>
                  </div>
                  <div className="text-right">
                    <p className="text-xs text-gray-400 uppercase tracking-wide">Amount</p>
                    <p className="text-xl font-bold text-gray-900 mt-0.5">{fmt(c.amount)}</p>
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-4 pt-3 border-t border-gray-200">
                  <SummaryField label="Category" value={c.category || '\u2014'} />
                  <SummaryField label="Expense Date" value={formatDate(c.expense_date)} />
                </div>
              </div>

              {/* Description */}
              {c.description && (
                <div>
                  <SummaryField label="Description" value={c.description} />
                </div>
              )}

              {/* Receipt */}
              {c.receipt_url && (
                <div>
                  <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">Receipt</p>
                  <AttachmentPreview url={c.receipt_url} name={c.receipt_file_name} />
                </div>
              )}

              {/* Review Notes (if already reviewed) */}
              {c.review_notes && (
                <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                  <SummaryField label="Review Notes" value={c.review_notes} />
                  {c.reviewed_at && (
                    <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(c.reviewed_at)}</p>
                  )}
                </div>
              )}

              {/* Notes input for pending items */}
              {c.status === 'pending' && (
                <div>
                  <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                  <textarea
                    placeholder="Add notes for your decision (optional)..."
                    value={reviewNotes[c.id] || ''}
                    onChange={(e) => setReviewNotes((prev) => ({ ...prev, [c.id]: e.target.value }))}
                    rows={2}
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                  />
                </div>
              )}
            </div>
          )}
          renderSummaryFooter={(c, close) =>
            c.status === 'pending' ? (
              <div className="flex items-center justify-end gap-3">
                <button
                  onClick={() => {
                    rejectClaimM.mutate({ id: c.id, notes: reviewNotes[c.id] }, { onSuccess: close });
                  }}
                  disabled={rejectClaimM.isPending}
                  className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                >
                  <XCircle className="w-4 h-4" />
                  {rejectClaimM.isPending ? 'Rejecting...' : 'Reject'}
                </button>
                <button
                  onClick={() => {
                    approveClaimM.mutate({ id: c.id, notes: reviewNotes[c.id] }, { onSuccess: close });
                  }}
                  disabled={approveClaimM.isPending}
                  className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                >
                  <CheckCircle className="w-4 h-4" />
                  {approveClaimM.isPending ? 'Approving...' : 'Approve'}
                </button>
              </div>
            ) : null
          }
        />
      )}

      {tab === 'overtime' && (
        <DataTable
          columns={overtimeColumns}
          data={overtimeQuery.data ?? []}
          perPage={10}
          isLoading={overtimeQuery.isLoading}
          emptyMessage="No overtime applications found"
          emptyIcon={<Clock className="w-8 h-8 text-gray-200" />}
          summaryTitle={(o) => `${o.employee_name} — Overtime Application`}
          renderSummary={(o) => (
            <div className="space-y-5">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                    {(o.employee_name || '').split(' ').map(n => n[0]).slice(0, 2).join('')}
                  </div>
                  <div>
                    <div className="font-semibold text-lg text-gray-900">{o.employee_name}</div>
                    <div className="text-sm text-gray-400">{o.employee_number}</div>
                  </div>
                </div>
                {statusBadge(o.status)}
              </div>

              <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
                <SummaryField label="Date" value={formatDate(o.ot_date)} />
                <SummaryField label="Type">
                  <span className={`text-sm font-medium mt-0.5 inline-block ${
                    o.ot_type === 'public_holiday' ? 'text-red-700' :
                    o.ot_type === 'rest_day' ? 'text-blue-700' : 'text-gray-900'
                  }`}>
                    {otTypeLabel(o.ot_type)} ({otTypeMultiplier(o.ot_type)})
                  </span>
                </SummaryField>
                <SummaryField label="Time" value={`${o.start_time?.slice(0, 5)} — ${o.end_time?.slice(0, 5)}`} />
                <SummaryField label="Duration" value={`${o.hours} hours`} />
              </div>

              {o.reason && <SummaryField label="Reason" value={o.reason} />}

              {o.review_notes && (
                <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                  <SummaryField label="Review Notes" value={o.review_notes} />
                  {o.reviewed_at && (
                    <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(o.reviewed_at)}</p>
                  )}
                </div>
              )}

              {o.status === 'pending' && (
                <div>
                  <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                  <textarea
                    placeholder="Add notes for your decision (optional)..."
                    value={reviewNotes[o.id] || ''}
                    onChange={(e) => setReviewNotes((prev) => ({ ...prev, [o.id]: e.target.value }))}
                    rows={2}
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                  />
                </div>
              )}
            </div>
          )}
          renderSummaryFooter={(o, close) =>
            o.status === 'pending' ? (
              <div className="flex items-center justify-end gap-3">
                <button
                  onClick={() => {
                    rejectOvertimeM.mutate({ id: o.id, notes: reviewNotes[o.id] }, { onSuccess: close });
                  }}
                  disabled={rejectOvertimeM.isPending}
                  className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                >
                  <XCircle className="w-4 h-4" />
                  {rejectOvertimeM.isPending ? 'Rejecting...' : 'Reject'}
                </button>
                <button
                  onClick={() => {
                    approveOvertimeM.mutate({ id: o.id, notes: reviewNotes[o.id] }, { onSuccess: close });
                  }}
                  disabled={approveOvertimeM.isPending}
                  className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 text-white text-sm font-medium rounded-lg hover:bg-emerald-700 disabled:opacity-50 transition-colors shadow-sm"
                >
                  <CheckCircle className="w-4 h-4" />
                  {approveOvertimeM.isPending ? 'Approving...' : 'Approve'}
                </button>
              </div>
            ) : null
          }
        />
      )}
    </div>
  );
}
