import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  CheckCircle,
  Clock,
  ExternalLink,
  FileText,
  Filter,
  Image,
  Paperclip,
  Pencil,
  Plus,
  Trash2,
  Upload,
  X,
  XCircle,
} from 'lucide-react';
import {
  approveClaim,
  approveLeave,
  approveOvertime,
  createClaim,
  createLeaveRequest,
  createOvertimeRequest,
  deleteClaim,
  deleteLeaveRequest,
  deleteOvertimeRequest,
  getClaims,
  getLeaveRequests,
  getOvertimeRequests,
  rejectClaim,
  rejectLeave,
  rejectOvertime,
  updateClaim,
  updateLeaveRequest,
  updateOvertimeRequest,
  type ClaimWithEmployee,
  type LeaveRequestWithEmployee,
  type OvertimeWithEmployee,
} from '@/api/approvals';
import { getEmployees } from '@/api/employees';
import { getLeaveTypes, uploadFile } from '@/api/portal';
import { Modal } from '@/components/ui/Modal';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { useAuth } from '@/context/AuthContext';
import { formatDate, getErrorMessage } from '@/lib/utils';
import type {
  AdminCreateClaimRequest,
  AdminCreateLeaveRequest,
  AdminCreateOvertimeRequest,
  CreateOvertimeRequest,
  Employee,
  LeaveType,
  UpdateClaimRequest,
  UpdateLeaveRequest,
  UpdateOvertimeRequest,
} from '@/types';

const fmt = (sen: number) => `RM ${(sen / 100).toFixed(2)}`;

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending',
    approved: 'badge-approved',
    rejected: 'badge-rejected',
    cancelled: 'badge-cancelled',
    draft: 'badge-draft',
    processed: 'badge-processed',
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

const canEditLeave = (request: LeaveRequestWithEmployee) => request.status === 'pending';
const canDeleteLeave = (request: LeaveRequestWithEmployee) => request.status === 'pending';
const canEditClaim = (claim: ClaimWithEmployee) => ['draft', 'pending', 'rejected'].includes(claim.status);
const canDeleteClaim = (claim: ClaimWithEmployee) => ['draft', 'pending', 'rejected'].includes(claim.status);
const canEditOvertime = (request: OvertimeWithEmployee) => request.status === 'pending';
const canDeleteOvertime = (request: OvertimeWithEmployee) => ['pending', 'rejected', 'cancelled'].includes(request.status);

function ActionButtons({
  onEdit,
  onDelete,
}: {
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  if (!onEdit && !onDelete) {
    return <span className="text-gray-300">—</span>;
  }

  return (
    <div className="flex items-center justify-center gap-2">
      {onEdit && (
        <button
          type="button"
          onClick={onEdit}
          className="p-1.5 rounded-lg text-gray-500 hover:text-black hover:bg-gray-100 transition-colors"
          title="Edit"
        >
          <Pencil className="w-4 h-4" />
        </button>
      )}
      {onDelete && (
        <button
          type="button"
          onClick={onDelete}
          className="p-1.5 rounded-lg text-red-500 hover:text-red-700 hover:bg-red-50 transition-colors"
          title="Delete"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}

export function Approvals() {
  const { user } = useAuth();
  const [tab, setTab] = useState<'leave' | 'claims' | 'overtime'>('leave');
  const [statusFilter, setStatusFilter] = useState<string>('pending');
  const [reviewNotes, setReviewNotes] = useState<Record<string, string>>({});
  const [leaveEditor, setLeaveEditor] = useState<LeaveRequestWithEmployee | null>(null);
  const [claimEditor, setClaimEditor] = useState<ClaimWithEmployee | null>(null);
  const [overtimeEditor, setOvertimeEditor] = useState<OvertimeWithEmployee | null>(null);
  const [showLeaveModal, setShowLeaveModal] = useState(false);
  const [showClaimModal, setShowClaimModal] = useState(false);
  const [showOvertimeModal, setShowOvertimeModal] = useState(false);
  const queryClient = useQueryClient();
  const activeCompanyId = user?.company_id ?? null;

  const { data: employeeResp } = useQuery({
    queryKey: ['approval-employees', activeCompanyId],
    queryFn: () => getEmployees({ is_active: true, page: 1, per_page: 100 }),
    enabled: Boolean(activeCompanyId),
  });

  const { data: leaveTypes = [] } = useQuery({
    queryKey: ['approval-leave-types'],
    queryFn: getLeaveTypes,
    enabled: tab === 'leave' || showLeaveModal,
  });

  const employees = useMemo(
    () => (employeeResp?.data ?? []).filter((employee) => employee.company_id === activeCompanyId),
    [activeCompanyId, employeeResp?.data],
  );

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

  const overtimeQuery = useQuery({
    queryKey: ['approvals-overtime', statusFilter],
    queryFn: () => getOvertimeRequests(statusFilter || undefined),
    enabled: tab === 'overtime',
  });

  const refreshLeave = () => queryClient.invalidateQueries({ queryKey: ['approvals-leave'] });
  const refreshClaims = () => queryClient.invalidateQueries({ queryKey: ['approvals-claims'] });
  const refreshOvertime = () => queryClient.invalidateQueries({ queryKey: ['approvals-overtime'] });

  const approveLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveLeave(id, notes),
    onSuccess: () => refreshLeave(),
  });

  const rejectLeaveM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectLeave(id, notes),
    onSuccess: () => refreshLeave(),
  });

  const approveClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveClaim(id, notes),
    onSuccess: () => refreshClaims(),
  });

  const rejectClaimM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectClaim(id, notes),
    onSuccess: () => refreshClaims(),
  });

  const approveOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => approveOvertime(id, notes),
    onSuccess: () => refreshOvertime(),
  });

  const rejectOvertimeM = useMutation({
    mutationFn: ({ id, notes }: { id: string; notes?: string }) => rejectOvertime(id, notes),
    onSuccess: () => refreshOvertime(),
  });

  const deleteLeaveM = useMutation({
    mutationFn: deleteLeaveRequest,
    onSuccess: () => refreshLeave(),
  });

  const deleteClaimM = useMutation({
    mutationFn: deleteClaim,
    onSuccess: () => refreshClaims(),
  });

  const deleteOvertimeM = useMutation({
    mutationFn: deleteOvertimeRequest,
    onSuccess: () => refreshOvertime(),
  });

  const otTypeLabel = (type: string) => {
    const labels: Record<string, string> = {
      normal: 'Normal Day',
      rest_day: 'Rest Day',
      public_holiday: 'Public Holiday',
    };
    return labels[type] || type;
  };

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

  const leaveColumns: Column<LeaveRequestWithEmployee>[] = [
    {
      key: 'employee',
      header: 'Employee',
      render: (request) => (
        <div>
          <div className="font-semibold text-gray-900">{request.employee_name}</div>
          <div className="text-xs text-gray-400">{request.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    { key: 'type', header: 'Leave Type', render: (request) => request.leave_type_name || '\u2014', primary: true },
    { key: 'period', header: 'Period', render: (request) => <span className="text-gray-500">{request.start_date} → {request.end_date}</span> },
    { key: 'days', header: 'Days', render: (request) => request.days },
    { key: 'reason', header: 'Reason', render: (request) => <span className="text-gray-400 max-w-[180px] truncate block">{request.reason || '\u2014'}</span> },
    {
      key: 'attachment',
      header: 'Attachment',
      render: (request) =>
        request.attachment_url ? (
          <span className="inline-flex items-center gap-1 text-gray-900 text-sm">
            <Paperclip className="w-3 h-3" />
            <span className="truncate max-w-[80px]">{request.attachment_name || 'File'}</span>
          </span>
        ) : (
          <span className="text-gray-300">\u2014</span>
        ),
    },
    { key: 'status', header: 'Status', render: (request) => statusBadge(request.status) },
  ];

  const claimColumns: Column<ClaimWithEmployee>[] = [
    {
      key: 'employee',
      header: 'Employee',
      render: (claim) => (
        <div>
          <div className="font-semibold text-gray-900">{claim.employee_name}</div>
          <div className="text-xs text-gray-400">{claim.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    {
      key: 'title',
      header: 'Title',
      render: (claim) => (
        <div>
          <div className="font-medium">{claim.title}</div>
          {claim.description && <div className="text-xs text-gray-400 truncate max-w-[180px]">{claim.description}</div>}
        </div>
      ),
      primary: true,
    },
    { key: 'category', header: 'Category', render: (claim) => claim.category || '\u2014' },
    { key: 'amount', header: 'Amount', align: 'right', render: (claim) => <span className="font-semibold">{fmt(claim.amount)}</span> },
    { key: 'date', header: 'Date', render: (claim) => <span className="text-gray-500">{claim.expense_date}</span> },
    {
      key: 'receipt',
      header: 'Receipt',
      render: (claim) =>
        claim.receipt_url ? (
          <a
            href={claim.receipt_url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-gray-900 hover:text-black text-sm"
          >
            {isImageUrl(claim.receipt_url) ? <Image className="w-3 h-3" /> : <Paperclip className="w-3 h-3" />}
            <span className="truncate max-w-[80px]">{claim.receipt_file_name || 'Receipt'}</span>
            <ExternalLink className="w-3 h-3" />
          </a>
        ) : (
          <span className="text-gray-300">\u2014</span>
        ),
    },
    { key: 'status', header: 'Status', render: (claim) => statusBadge(claim.status) },
  ];

  const overtimeColumns: Column<OvertimeWithEmployee>[] = [
    {
      key: 'employee',
      header: 'Employee',
      render: (overtime) => (
        <div>
          <div className="font-semibold text-gray-900">{overtime.employee_name}</div>
          <div className="text-xs text-gray-400">{overtime.employee_number}</div>
        </div>
      ),
      primary: true,
    },
    { key: 'date', header: 'Date', render: (overtime) => <span className="text-gray-700">{overtime.ot_date}</span>, primary: true },
    {
      key: 'type',
      header: 'Type',
      render: (overtime) => (
        <span
          className={`text-xs px-2 py-0.5 rounded-full font-medium ${
            overtime.ot_type === 'public_holiday'
              ? 'bg-red-100 text-red-700'
              : overtime.ot_type === 'rest_day'
                ? 'bg-blue-100 text-blue-700'
                : 'bg-gray-100 text-gray-600'
          }`}
        >
          {otTypeLabel(overtime.ot_type)} ({otTypeMultiplier(overtime.ot_type)})
        </span>
      ),
    },
    {
      key: 'time',
      header: 'Time',
      render: (overtime) => <span className="text-gray-500">{overtime.start_time?.slice(0, 5)} — {overtime.end_time?.slice(0, 5)}</span>,
    },
    { key: 'hours', header: 'Hours', align: 'right', render: (overtime) => <span className="font-semibold">{overtime.hours}h</span> },
    { key: 'reason', header: 'Reason', render: (overtime) => <span className="text-gray-400 max-w-[180px] truncate block">{overtime.reason || '\u2014'}</span> },
    { key: 'status', header: 'Status', render: (overtime) => statusBadge(overtime.status) },
  ];

  const headerButtonLabel = useMemo(() => {
    if (tab === 'leave') return 'Add Leave';
    if (tab === 'claims') return 'Add Claim';
    return 'Add Overtime';
  }, [tab]);

  return (
    <>
      <div className="space-y-6">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="page-header">
            <h1 className="page-title">Approvals</h1>
            <p className="page-subtitle">Review requests and manage leave, claims, and overtime for your company.</p>
          </div>
          <button onClick={openCreateModal} className="btn-primary w-full sm:w-auto">
            <Plus className="w-4 h-4" /> {headerButtonLabel}
          </button>
        </div>

        <div className="flex gap-1 border-b border-gray-200 overflow-x-auto">
          {(['leave', 'claims', 'overtime'] as const).map((itemTab) => (
            <button
              key={itemTab}
              onClick={() => {
                setTab(itemTab);
                setStatusFilter('pending');
              }}
              className={`px-5 py-2.5 text-sm font-medium border-b-2 transition-all-fast ${
                tab === itemTab ? 'border-black text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
              }`}
            >
              {itemTab === 'leave' ? 'Leave Requests' : itemTab === 'claims' ? 'Expense Claims' : 'Overtime'}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <Filter className="w-4 h-4 text-gray-300" />
          {['pending', 'approved', 'rejected', 'cancelled', ''].map((status) => (
            <button
              key={status}
              onClick={() => setStatusFilter(status)}
              className={`px-3.5 py-1.5 text-xs font-medium rounded-full transition-all-fast ${
                statusFilter === status
                  ? 'bg-black text-white shadow-sm'
                  : 'bg-white text-gray-500 border border-gray-200 hover:border-gray-300'
              }`}
            >
              {status || 'All'}
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
            renderActions={(request) => (
              <ActionButtons
                onEdit={canEditLeave(request) ? () => {
                  setLeaveEditor(request);
                  setShowLeaveModal(true);
                } : undefined}
                onDelete={canDeleteLeave(request) ? () => {
                  if (confirm('Delete this leave request?')) {
                    deleteLeaveM.mutate(request.id);
                  }
                } : undefined}
              />
            )}
            summaryTitle={(request) => `${request.employee_name} — Leave Request`}
            renderSummary={(request) => (
              <div className="space-y-5">
                <div className="flex items-start justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                      {(request.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div>
                      <div className="font-semibold text-lg text-gray-900">{request.employee_name}</div>
                      <div className="text-sm text-gray-400">{request.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(request.status)}
                </div>

                <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
                  <SummaryField label="Leave Type" value={request.leave_type_name || '\u2014'} />
                  <SummaryField label="Duration" value={`${request.days} day(s)`} />
                  <SummaryField label="Start Date" value={formatDate(request.start_date)} />
                  <SummaryField label="End Date" value={formatDate(request.end_date)} />
                </div>

                {request.reason && <SummaryField label="Reason" value={request.reason} />}

                {request.attachment_url && (
                  <div>
                    <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">Attachment</p>
                    <AttachmentPreview url={request.attachment_url} name={request.attachment_name} />
                  </div>
                )}

                {request.review_notes && (
                  <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                    <SummaryField label="Review Notes" value={request.review_notes} />
                    {request.reviewed_at && <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(request.reviewed_at)}</p>}
                  </div>
                )}

                {request.status === 'pending' && (
                  <div>
                    <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                    <textarea
                      placeholder="Add notes for your decision (optional)..."
                      value={reviewNotes[request.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [request.id]: event.target.value }))}
                      rows={2}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={(request, close) =>
              request.status === 'pending' ? (
                <div className="flex items-center justify-end gap-3">
                    <button
                      onClick={() => rejectLeaveM.mutate({ id: request.id, notes: reviewNotes[request.id] }, { onSuccess: close })}
                      disabled={rejectLeaveM.isPending}
                      className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                    >
                      <XCircle className="w-4 h-4" />
                      {rejectLeaveM.isPending ? 'Rejecting...' : 'Reject'}
                    </button>
                    <button
                      onClick={() => approveLeaveM.mutate({ id: request.id, notes: reviewNotes[request.id] }, { onSuccess: close })}
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
            renderActions={(claim) => (
              <ActionButtons
                onEdit={canEditClaim(claim) ? () => {
                  setClaimEditor(claim);
                  setShowClaimModal(true);
                } : undefined}
                onDelete={canDeleteClaim(claim) ? () => {
                  if (confirm('Delete this claim?')) {
                    deleteClaimM.mutate(claim.id);
                  }
                } : undefined}
              />
            )}
            summaryTitle={(claim) => `${claim.employee_name} — Expense Claim`}
            renderSummary={(claim) => (
              <div className="space-y-5">
                <div className="flex items-start justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                      {(claim.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div>
                      <div className="font-semibold text-lg text-gray-900">{claim.employee_name}</div>
                      <div className="text-sm text-gray-400">{claim.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(claim.status)}
                </div>

                <div className="bg-gray-50 rounded-lg p-4 space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs text-gray-400 uppercase tracking-wide">Claim Title</p>
                      <p className="text-base font-semibold text-gray-900 mt-0.5">{claim.title}</p>
                    </div>
                    <div className="text-right">
                      <p className="text-xs text-gray-400 uppercase tracking-wide">Amount</p>
                      <p className="text-xl font-bold text-gray-900 mt-0.5">{fmt(claim.amount)}</p>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-4 pt-3 border-t border-gray-200">
                    <SummaryField label="Category" value={claim.category || '\u2014'} />
                    <SummaryField label="Expense Date" value={formatDate(claim.expense_date)} />
                  </div>
                </div>

                {claim.description && <SummaryField label="Description" value={claim.description} />}

                {claim.receipt_url && (
                  <div>
                    <p className="text-xs text-gray-400 uppercase tracking-wide mb-1">Receipt</p>
                    <AttachmentPreview url={claim.receipt_url} name={claim.receipt_file_name} />
                  </div>
                )}

                {claim.review_notes && (
                  <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                    <SummaryField label="Review Notes" value={claim.review_notes} />
                    {claim.reviewed_at && <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(claim.reviewed_at)}</p>}
                  </div>
                )}

                {claim.status === 'pending' && (
                  <div>
                    <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                    <textarea
                      placeholder="Add notes for your decision (optional)..."
                      value={reviewNotes[claim.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [claim.id]: event.target.value }))}
                      rows={2}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={(claim, close) =>
              claim.status === 'pending' ? (
                <div className="flex items-center justify-end gap-3">
                    <button
                      onClick={() => rejectClaimM.mutate({ id: claim.id, notes: reviewNotes[claim.id] }, { onSuccess: close })}
                      disabled={rejectClaimM.isPending}
                      className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                    >
                      <XCircle className="w-4 h-4" />
                      {rejectClaimM.isPending ? 'Rejecting...' : 'Reject'}
                    </button>
                    <button
                      onClick={() => approveClaimM.mutate({ id: claim.id, notes: reviewNotes[claim.id] }, { onSuccess: close })}
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
            renderActions={(overtime) => (
              <ActionButtons
                onEdit={canEditOvertime(overtime) ? () => {
                  setOvertimeEditor(overtime);
                  setShowOvertimeModal(true);
                } : undefined}
                onDelete={canDeleteOvertime(overtime) ? () => {
                  if (confirm('Delete this overtime application?')) {
                    deleteOvertimeM.mutate(overtime.id);
                  }
                } : undefined}
              />
            )}
            summaryTitle={(overtime) => `${overtime.employee_name} — Overtime Application`}
            renderSummary={(overtime) => (
              <div className="space-y-5">
                <div className="flex items-start justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm">
                      {(overtime.employee_name || '').split(' ').map((name) => name[0]).slice(0, 2).join('')}
                    </div>
                    <div>
                      <div className="font-semibold text-lg text-gray-900">{overtime.employee_name}</div>
                      <div className="text-sm text-gray-400">{overtime.employee_number}</div>
                    </div>
                  </div>
                  {statusBadge(overtime.status)}
                </div>

                <div className="grid grid-cols-2 gap-4 bg-gray-50 rounded-lg p-4">
                  <SummaryField label="Date" value={formatDate(overtime.ot_date)} />
                  <SummaryField label="Type">
                    <span
                      className={`text-sm font-medium mt-0.5 inline-block ${
                        overtime.ot_type === 'public_holiday'
                          ? 'text-red-700'
                          : overtime.ot_type === 'rest_day'
                            ? 'text-blue-700'
                            : 'text-gray-900'
                      }`}
                    >
                      {otTypeLabel(overtime.ot_type)} ({otTypeMultiplier(overtime.ot_type)})
                    </span>
                  </SummaryField>
                  <SummaryField label="Time" value={`${overtime.start_time?.slice(0, 5)} — ${overtime.end_time?.slice(0, 5)}`} />
                  <SummaryField label="Duration" value={`${overtime.hours} hours`} />
                </div>

                {overtime.reason && <SummaryField label="Reason" value={overtime.reason} />}

                {overtime.review_notes && (
                  <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
                    <SummaryField label="Review Notes" value={overtime.review_notes} />
                    {overtime.reviewed_at && <p className="text-xs text-gray-400 mt-1">Reviewed on {formatDate(overtime.reviewed_at)}</p>}
                  </div>
                )}

                {overtime.status === 'pending' && (
                  <div>
                    <label className="text-xs text-gray-400 uppercase tracking-wide block mb-1">Review Notes</label>
                    <textarea
                      placeholder="Add notes for your decision (optional)..."
                      value={reviewNotes[overtime.id] || ''}
                      onChange={(event) => setReviewNotes((prev) => ({ ...prev, [overtime.id]: event.target.value }))}
                      rows={2}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black outline-none text-sm resize-none"
                    />
                  </div>
                )}
              </div>
            )}
            renderSummaryFooter={(overtime, close) =>
              overtime.status === 'pending' ? (
                <div className="flex items-center justify-end gap-3">
                    <button
                      onClick={() => rejectOvertimeM.mutate({ id: overtime.id, notes: reviewNotes[overtime.id] }, { onSuccess: close })}
                      disabled={rejectOvertimeM.isPending}
                      className="flex items-center gap-2 px-4 py-2.5 bg-white border border-red-200 text-red-600 text-sm font-medium rounded-lg hover:bg-red-50 hover:border-red-300 disabled:opacity-50 transition-colors"
                    >
                      <XCircle className="w-4 h-4" />
                      {rejectOvertimeM.isPending ? 'Rejecting...' : 'Reject'}
                    </button>
                    <button
                      onClick={() => approveOvertimeM.mutate({ id: overtime.id, notes: reviewNotes[overtime.id] }, { onSuccess: close })}
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

      <LeaveCrudModal
        open={showLeaveModal}
        onClose={() => {
          setShowLeaveModal(false);
          setLeaveEditor(null);
        }}
        item={leaveEditor}
        employees={employees}
        leaveTypes={leaveTypes}
        onSaved={() => {
          setShowLeaveModal(false);
          setLeaveEditor(null);
          refreshLeave();
        }}
      />

      <ClaimCrudModal
        open={showClaimModal}
        onClose={() => {
          setShowClaimModal(false);
          setClaimEditor(null);
        }}
        item={claimEditor}
        employees={employees}
        onSaved={() => {
          setShowClaimModal(false);
          setClaimEditor(null);
          refreshClaims();
        }}
      />

      <OvertimeCrudModal
        open={showOvertimeModal}
        onClose={() => {
          setShowOvertimeModal(false);
          setOvertimeEditor(null);
        }}
        item={overtimeEditor}
        employees={employees}
        onSaved={() => {
          setShowOvertimeModal(false);
          setOvertimeEditor(null);
          refreshOvertime();
        }}
      />
    </>
  );
}

function LeaveCrudModal({
  open,
  onClose,
  item,
  employees,
  leaveTypes,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: LeaveRequestWithEmployee | null;
  employees: Employee[];
  leaveTypes: LeaveType[];
  onSaved: () => void;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState<AdminCreateLeaveRequest>({
    employee_id: '',
    leave_type_id: '',
    start_date: '',
    end_date: '',
    days: 1,
    reason: '',
    attachment_url: '',
    attachment_name: '',
  });
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const selectedEmployee = employees.find((employee) => employee.id === form.employee_id);
  const modalTitle = item
    ? `Edit Leave Request${selectedEmployee ? ` - ${selectedEmployee.full_name}` : ''}`
    : 'Create Leave Request';

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        leave_type_id: item.leave_type_id,
        start_date: item.start_date,
        end_date: item.end_date,
        days: Number(item.days),
        reason: item.reason || '',
        attachment_url: item.attachment_url || '',
        attachment_name: item.attachment_name || '',
      });
      setError('');
      return;
    }

    setForm({
      employee_id: employees[0]?.id || '',
      leave_type_id: leaveTypes[0]?.id || '',
      start_date: '',
      end_date: '',
      days: 1,
      reason: '',
      attachment_url: '',
      attachment_name: '',
    });
    setError('');
  }, [employees, item, leaveTypes, open]);

  const createMutation = useMutation({
    mutationFn: createLeaveRequest,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to create leave request')),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateLeaveRequest }) => updateLeaveRequest(id, payload),
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to update leave request')),
  });

  const handleUpload = async (file: File) => {
    setError('');
    setUploading(true);
    try {
      const result = await uploadFile(file);
      setForm((prev) => ({
        ...prev,
        attachment_url: result.url,
        attachment_name: result.file_name,
      }));
    } catch (err: unknown) {
      setError(getErrorMessage(err, 'Failed to upload attachment'));
    } finally {
      setUploading(false);
    }
  };

  const submit = () => {
    if (!form.employee_id || !form.leave_type_id || !form.start_date || !form.end_date || form.days <= 0) {
      setError('Employee, leave type, dates, and days are required.');
      return;
    }

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          leave_type_id: form.leave_type_id,
          start_date: form.start_date,
          end_date: form.end_date,
          days: form.days,
          reason: form.reason,
          attachment_url: form.attachment_url,
          attachment_name: form.attachment_name,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      reason: form.reason || undefined,
      attachment_url: form.attachment_url || undefined,
      attachment_name: form.attachment_name || undefined,
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={modalTitle}
      footer={
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button
            onClick={submit}
            disabled={createMutation.isPending || updateMutation.isPending || uploading}
            className="btn-primary"
          >
            {createMutation.isPending || updateMutation.isPending ? 'Saving...' : item ? 'Save Changes' : 'Create Request'}
          </button>
        </div>
      }
    >
      <div className="space-y-4">
        {error && <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{error}</div>}

        <div>
          <label className="form-label">Employee *</label>
          <select
            value={form.employee_id}
            onChange={(event) => setForm((prev) => ({ ...prev, employee_id: event.target.value }))}
            className="form-input"
          >
            <option value="">Select employee</option>
            {employees.map((employee) => (
              <option key={employee.id} value={employee.id}>
                {employee.full_name} ({employee.employee_number})
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="form-label">Leave Type *</label>
          <select
            value={form.leave_type_id}
            onChange={(event) => setForm((prev) => ({ ...prev, leave_type_id: event.target.value }))}
            className="form-input"
          >
            <option value="">Select leave type</option>
            {leaveTypes.map((leaveType) => (
              <option key={leaveType.id} value={leaveType.id}>{leaveType.name}</option>
            ))}
          </select>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="form-label">Start Date *</label>
            <input
              type="date"
              value={form.start_date}
              onChange={(event) => setForm((prev) => ({ ...prev, start_date: event.target.value }))}
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">End Date *</label>
            <input
              type="date"
              value={form.end_date}
              onChange={(event) => setForm((prev) => ({ ...prev, end_date: event.target.value }))}
              className="form-input"
            />
          </div>
        </div>

        <div>
          <label className="form-label">Days *</label>
          <input
            type="number"
            min="0.5"
            step="0.5"
            value={form.days}
            onChange={(event) => setForm((prev) => ({ ...prev, days: Number(event.target.value) }))}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">Reason</label>
          <textarea
            value={form.reason}
            onChange={(event) => setForm((prev) => ({ ...prev, reason: event.target.value }))}
            rows={3}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">Attachment</label>
          <input
            ref={fileInputRef}
            type="file"
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void handleUpload(file);
              }
            }}
          />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="btn-secondary"
              disabled={uploading}
            >
              <Upload className="w-4 h-4" /> {uploading ? 'Uploading...' : 'Upload File'}
            </button>
            {form.attachment_name && (
              <button
                type="button"
                onClick={() => setForm((prev) => ({ ...prev, attachment_url: '', attachment_name: '' }))}
                className="p-2 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          {form.attachment_name && <p className="text-sm text-gray-500 mt-2">{form.attachment_name}</p>}
        </div>
      </div>
    </Modal>
  );
}

function ClaimCrudModal({
  open,
  onClose,
  item,
  employees,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: ClaimWithEmployee | null;
  employees: Employee[];
  onSaved: () => void;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();
  const [form, setForm] = useState<AdminCreateClaimRequest>({
    employee_id: '',
    title: '',
    description: '',
    amount: 0,
    category: '',
    receipt_url: '',
    receipt_file_name: '',
    expense_date: new Date().toISOString().slice(0, 10),
  });
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const selectedEmployee = employees.find((employee) => employee.id === form.employee_id);
  const modalTitle = item
    ? `Edit Claim${selectedEmployee ? ` - ${selectedEmployee.full_name}` : ''}`
    : 'Create Claim';

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        title: item.title,
        description: item.description || '',
        amount: item.amount / 100,
        category: item.category || '',
        receipt_url: item.receipt_url || '',
        receipt_file_name: item.receipt_file_name || '',
        expense_date: item.expense_date,
      });
      setError('');
      return;
    }

    setForm({
      employee_id: employees[0]?.id || '',
      title: '',
      description: '',
      amount: 0,
      category: '',
      receipt_url: '',
      receipt_file_name: '',
      expense_date: new Date().toISOString().slice(0, 10),
    });
    setError('');
  }, [employees, item, open]);

  const createMutation = useMutation({
    mutationFn: createClaim,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to create claim')),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateClaimRequest }) => updateClaim(id, payload),
    onSuccess: async (updatedClaim) => {
      queryClient.setQueriesData<ClaimWithEmployee[]>({ queryKey: ['approvals-claims'] }, (current) =>
        current?.map((claim) =>
          claim.id === updatedClaim.id
            ? {
                ...claim,
                ...updatedClaim,
              }
            : claim
        ) ?? current
      );
      await queryClient.refetchQueries({ queryKey: ['approvals-claims'] });
      onSaved();
    },
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to update claim')),
  });

  const handleUpload = async (file: File) => {
    setError('');
    setUploading(true);
    try {
      const result = await uploadFile(file);
      setForm((prev) => ({
        ...prev,
        receipt_url: result.url,
        receipt_file_name: result.file_name,
      }));
    } catch (err: unknown) {
      setError(getErrorMessage(err, 'Failed to upload receipt'));
    } finally {
      setUploading(false);
    }
  };

  const submit = () => {
    if (!form.employee_id || !form.title || !form.expense_date || form.amount <= 0) {
      setError('Employee, title, amount, and expense date are required.');
      return;
    }

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          title: form.title,
          description: form.description,
          amount: Math.round(form.amount * 100),
          category: form.category,
          receipt_url: form.receipt_url,
          receipt_file_name: form.receipt_file_name,
          expense_date: form.expense_date,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      amount: Math.round(form.amount * 100),
      description: form.description || undefined,
      category: form.category || undefined,
      receipt_url: form.receipt_url || undefined,
      receipt_file_name: form.receipt_file_name || undefined,
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={modalTitle}
      footer={
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button
            onClick={submit}
            disabled={createMutation.isPending || updateMutation.isPending || uploading}
            className="btn-primary"
          >
            {createMutation.isPending || updateMutation.isPending ? 'Saving...' : item ? 'Save Changes' : 'Create Claim'}
          </button>
        </div>
      }
    >
      <div className="space-y-4">
        {error && <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{error}</div>}

        <div>
          <label className="form-label">Employee *</label>
          <select
            value={form.employee_id}
            onChange={(event) => setForm((prev) => ({ ...prev, employee_id: event.target.value }))}
            className="form-input"
          >
            <option value="">Select employee</option>
            {employees.map((employee) => (
              <option key={employee.id} value={employee.id}>
                {employee.full_name} ({employee.employee_number})
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="form-label">Title *</label>
          <input
            type="text"
            value={form.title}
            onChange={(event) => setForm((prev) => ({ ...prev, title: event.target.value }))}
            className="form-input"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="form-label">Amount (RM) *</label>
            <input
              type="number"
              min="0.01"
              step="0.01"
              value={form.amount}
              onChange={(event) => setForm((prev) => ({ ...prev, amount: Number(event.target.value) }))}
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">Expense Date *</label>
            <input
              type="date"
              value={form.expense_date}
              onChange={(event) => setForm((prev) => ({ ...prev, expense_date: event.target.value }))}
              className="form-input"
            />
          </div>
        </div>

        <div>
          <label className="form-label">Category</label>
          <input
            type="text"
            value={form.category}
            onChange={(event) => setForm((prev) => ({ ...prev, category: event.target.value }))}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">Description</label>
          <textarea
            value={form.description}
            onChange={(event) => setForm((prev) => ({ ...prev, description: event.target.value }))}
            rows={3}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">Receipt</label>
          <input
            ref={fileInputRef}
            type="file"
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void handleUpload(file);
              }
            }}
          />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="btn-secondary"
              disabled={uploading}
            >
              <Upload className="w-4 h-4" /> {uploading ? 'Uploading...' : 'Upload Receipt'}
            </button>
            {form.receipt_file_name && (
              <button
                type="button"
                onClick={() => setForm((prev) => ({ ...prev, receipt_url: '', receipt_file_name: '' }))}
                className="p-2 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          {form.receipt_file_name && <p className="text-sm text-gray-500 mt-2">{form.receipt_file_name}</p>}
        </div>
      </div>
    </Modal>
  );
}

function OvertimeCrudModal({
  open,
  onClose,
  item,
  employees,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  item: OvertimeWithEmployee | null;
  employees: Employee[];
  onSaved: () => void;
}) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<AdminCreateOvertimeRequest>({
    employee_id: '',
    ot_date: '',
    start_time: '',
    end_time: '',
    hours: 0,
    ot_type: 'normal',
    reason: '',
  });
  const [error, setError] = useState('');
  const selectedEmployee = employees.find((employee) => employee.id === form.employee_id);
  const modalTitle = item
    ? `Edit Overtime Request${selectedEmployee ? ` - ${selectedEmployee.full_name}` : ''}`
    : 'Create Overtime Request';

  useEffect(() => {
    if (!open) {
      return;
    }

    if (item) {
      setForm({
        employee_id: item.employee_id,
        ot_date: item.ot_date,
        start_time: item.start_time?.slice(0, 5),
        end_time: item.end_time?.slice(0, 5),
        hours: Number(item.hours),
        ot_type: item.ot_type as CreateOvertimeRequest['ot_type'],
        reason: item.reason || '',
      });
      setError('');
      return;
    }

    setForm({
      employee_id: employees[0]?.id || '',
      ot_date: '',
      start_time: '',
      end_time: '',
      hours: 0,
      ot_type: 'normal',
      reason: '',
    });
    setError('');
  }, [employees, item, open]);

  const createMutation = useMutation({
    mutationFn: createOvertimeRequest,
    onSuccess: onSaved,
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to create overtime request')),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: UpdateOvertimeRequest }) => updateOvertimeRequest(id, payload),
    onSuccess: async (updatedOvertime) => {
      queryClient.setQueriesData<OvertimeWithEmployee[]>({ queryKey: ['approvals-overtime'] }, (current) =>
        current?.map((overtime) =>
          overtime.id === updatedOvertime.id
            ? {
                ...overtime,
                ...updatedOvertime,
              }
            : overtime
        ) ?? current
      );
      await queryClient.refetchQueries({ queryKey: ['approvals-overtime'] });
      onSaved();
    },
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to update overtime request')),
  });

  const calculateHours = (start: string, end: string) => {
    if (!start || !end) return 0;
    const [startHour, startMinute] = start.split(':').map(Number);
    const [endHour, endMinute] = end.split(':').map(Number);
    let diff = (endHour * 60 + endMinute) - (startHour * 60 + startMinute);
    if (diff <= 0) diff += 24 * 60;
    return Math.round(diff / 30) * 0.5;
  };

  const updateTime = (field: 'start_time' | 'end_time', value: string) => {
    setForm((prev) => {
      const next = { ...prev, [field]: value };
      next.hours = calculateHours(
        field === 'start_time' ? value : prev.start_time,
        field === 'end_time' ? value : prev.end_time,
      );
      return next;
    });
  };

  const submit = () => {
    if (!form.employee_id || !form.ot_date || !form.start_time || !form.end_time || form.hours <= 0) {
      setError('Employee, OT date, time range, and hours are required.');
      return;
    }

    if (item) {
      updateMutation.mutate({
        id: item.id,
        payload: {
          employee_id: form.employee_id,
          ot_date: form.ot_date,
          start_time: form.start_time,
          end_time: form.end_time,
          hours: form.hours,
          ot_type: form.ot_type,
          reason: form.reason,
        },
      });
      return;
    }

    createMutation.mutate({
      ...form,
      reason: form.reason || undefined,
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={modalTitle}
      footer={
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button
            onClick={submit}
            disabled={createMutation.isPending || updateMutation.isPending}
            className="btn-primary"
          >
            {createMutation.isPending || updateMutation.isPending ? 'Saving...' : item ? 'Save Changes' : 'Create Overtime'}
          </button>
        </div>
      }
    >
      <div className="space-y-4">
        {error && <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{error}</div>}

        <div>
          <label className="form-label">Employee *</label>
          <select
            value={form.employee_id}
            onChange={(event) => setForm((prev) => ({ ...prev, employee_id: event.target.value }))}
            className="form-input"
          >
            <option value="">Select employee</option>
            {employees.map((employee) => (
              <option key={employee.id} value={employee.id}>
                {employee.full_name} ({employee.employee_number})
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="form-label">OT Date *</label>
          <input
            type="date"
            value={form.ot_date}
            onChange={(event) => setForm((prev) => ({ ...prev, ot_date: event.target.value }))}
            className="form-input"
          />
        </div>

        <div>
          <label className="form-label">OT Type *</label>
          <div className="grid grid-cols-3 gap-2">
            {(['normal', 'rest_day', 'public_holiday'] as const).map((type) => (
              <label
                key={type}
                className={`flex flex-col items-center gap-1 px-3 py-3 rounded-lg border-2 cursor-pointer transition-all text-center ${
                  form.ot_type === type
                    ? type === 'public_holiday'
                      ? 'border-red-400 bg-red-50'
                      : type === 'rest_day'
                        ? 'border-blue-400 bg-blue-50'
                        : 'border-gray-900 bg-gray-50'
                    : 'border-gray-200 hover:border-gray-300'
                }`}
              >
                <input
                  type="radio"
                  checked={form.ot_type === type}
                  onChange={() => setForm((prev) => ({ ...prev, ot_type: type }))}
                  className="sr-only"
                />
                <span className="text-xs font-medium">{type === 'normal' ? 'Normal Day' : type === 'rest_day' ? 'Rest Day' : 'Public Holiday'}</span>
                <span className="text-[10px] text-gray-400">{type === 'normal' ? '1.5x' : type === 'rest_day' ? '2.0x' : '3.0x'}</span>
              </label>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <TimeSelector
            label="Start Time *"
            value={form.start_time || '18:00'}
            onChange={(value) => updateTime('start_time', value)}
          />
          <TimeSelector
            label="End Time *"
            value={form.end_time || '19:00'}
            onChange={(value) => updateTime('end_time', value)}
          />
        </div>

        {form.hours > 0 && (
          <div className="bg-gray-50 rounded-lg p-3 text-center">
            <span className="text-2xl font-bold text-gray-900">{form.hours}</span>
            <span className="text-sm text-gray-400 ml-1">hours</span>
          </div>
        )}

        <div>
          <label className="form-label">Reason</label>
          <textarea
            value={form.reason}
            onChange={(event) => setForm((prev) => ({ ...prev, reason: event.target.value }))}
            rows={3}
            className="form-input"
          />
        </div>
      </div>
    </Modal>
  );
}
