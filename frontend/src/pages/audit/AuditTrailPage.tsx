import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ScrollText, Search, X, ChevronLeft, ChevronRight } from 'lucide-react';
import { getAuditLogs } from '@/api/audit';
import type { AuditLog } from '@/types';

const ENTITY_TYPES: { value: string; label: string }[] = [
  { value: 'employee', label: 'Employee' },
  { value: 'payroll_run', label: 'Payroll run' },
  { value: 'payroll_item', label: 'Payroll item' },
  { value: 'attendance_record', label: 'Attendance record' },
  { value: 'platform_attendance_method', label: 'Platform attendance method' },
  { value: 'company_attendance_method', label: 'Company attendance method' },
  { value: 'attendance_kiosk_credential', label: 'Kiosk credential' },
  { value: 'company_location', label: 'Geofence location' },
  { value: 'geofence_mode', label: 'Geofence mode' },
  { value: 'work_schedule', label: 'Work schedule' },
  { value: 'leave_request', label: 'Leave request' },
  { value: 'claim', label: 'Claim' },
  { value: 'overtime', label: 'Overtime' },
  { value: 'company', label: 'Company' },
  { value: 'user', label: 'User' },
  { value: 'team', label: 'Team' },
  { value: 'document', label: 'Document' },
  { value: 'holiday', label: 'Holiday' },
  { value: 'setting', label: 'Setting' },
  { value: 'leave_type', label: 'Leave type' },
  { value: 'leave_balance', label: 'Leave balance' },
  { value: 'email', label: 'Email' },
];

const ACTIONS = ['create', 'update', 'delete', 'revoke', 'approve', 'reject', 'cancel', 'login', 'process'];

const ENTITY_LABELS = Object.fromEntries(ENTITY_TYPES.map((type) => [type.value, type.label]));

const formatEntityType = (entityType: string) =>
  ENTITY_LABELS[entityType] ?? entityType.replace(/_/g, ' ');

const formatTimestamp = (value: string) =>
  new Date(value).toLocaleString('en-MY', {
    dateStyle: 'medium',
    timeStyle: 'short',
  });

export function AuditTrailPage() {
  const [entityType, setEntityType] = useState('');
  const [action, setAction] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [page, setPage] = useState(1);
  const [selectedLog, setSelectedLog] = useState<AuditLog | null>(null);
  const perPage = 25;

  const { data, isLoading } = useQuery({
    queryKey: ['audit-logs', entityType, action, startDate, endDate, page],
    queryFn: () => getAuditLogs({
      entity_type: entityType || undefined,
      action: action || undefined,
      start_date: startDate || undefined,
      end_date: endDate || undefined,
      page,
      per_page: perPage,
    }),
  });

  const logs = data?.data ?? [];
  const total = data?.total ?? 0;
  const totalPages = Math.ceil(total / perPage);

  const clearFilters = () => {
    setEntityType('');
    setAction('');
    setStartDate('');
    setEndDate('');
    setPage(1);
  };

  const hasFilters = entityType || action || startDate || endDate;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">Audit Trail</h1>
          <p className="text-sm text-gray-500 mt-1">Track all changes and actions across the system</p>
        </div>
        <div className="flex items-center gap-2 text-sm text-gray-500">
          <ScrollText className="w-4 h-4" />
          {total} records
        </div>
      </div>

      {/* Filters */}
      <div className="bg-white rounded-xl border border-gray-200 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs font-medium text-gray-500 mb-1">Entity Type</label>
            <select
              value={entityType}
              onChange={(e) => { setEntityType(e.target.value); setPage(1); }}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-black/5"
            >
              <option value="">All types</option>
              {ENTITY_TYPES.map((type) => (
                <option key={type.value} value={type.value}>{type.label}</option>
              ))}
            </select>
          </div>
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs font-medium text-gray-500 mb-1">Action</label>
            <select
              value={action}
              onChange={(e) => { setAction(e.target.value); setPage(1); }}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-black/5"
            >
              <option value="">All actions</option>
              {ACTIONS.map(a => (
                <option key={a} value={a}>{a}</option>
              ))}
            </select>
          </div>
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs font-medium text-gray-500 mb-1">From</label>
            <input
              type="date"
              value={startDate}
              onChange={(e) => { setStartDate(e.target.value); setPage(1); }}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-black/5"
            />
          </div>
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs font-medium text-gray-500 mb-1">To</label>
            <input
              type="date"
              value={endDate}
              onChange={(e) => { setEndDate(e.target.value); setPage(1); }}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-black/5"
            />
          </div>
          {hasFilters && (
            <button
              onClick={clearFilters}
              className="px-3 py-2 text-sm text-gray-500 hover:text-gray-700 flex items-center gap-1"
            >
              <X className="w-3.5 h-3.5" /> Clear
            </button>
          )}
        </div>
      </div>

      {/* Table */}
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        {isLoading ? (
          <div className="flex items-center justify-center py-20 text-gray-400">
            <Search className="w-5 h-5 animate-spin mr-2" /> Loading...
          </div>
        ) : logs.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-400">
            <ScrollText className="w-8 h-8 mb-2" />
            <p>No audit logs found</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100 bg-gray-50/50">
                  <th className="text-left px-4 py-3 font-medium text-gray-500">Timestamp</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500">User</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500">Entity</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500">Action</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500">Details</th>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <tr
                    key={log.id}
                    onClick={() => setSelectedLog(log)}
                    className="border-b border-gray-50 hover:bg-gray-50/50 cursor-pointer transition-colors"
                  >
                    <td className="px-4 py-3 text-gray-600 whitespace-nowrap">
                      {formatTimestamp(log.created_at)}
                    </td>
                    <td className="px-4 py-3">
                      <div className="text-gray-900">{log.user_full_name || '-'}</div>
                      <div className="text-xs text-gray-400">{log.user_email}</div>
                    </td>
                    <td className="px-4 py-3">
                      <span className="inline-flex px-2 py-0.5 text-xs font-medium bg-gray-100 text-gray-700 rounded-full">
                        {formatEntityType(log.entity_type)}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <ActionBadge action={log.action} />
                    </td>
                    <td className="px-4 py-3 text-gray-500 text-xs max-w-[260px]">
                      <div className="truncate">{log.description || '-'}</div>
                      {log.entity_id && (
                        <div className="mt-0.5 font-mono text-[11px] text-gray-400">
                          {log.entity_id.slice(0, 8)}...
                        </div>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-gray-100">
            <p className="text-sm text-gray-500">
              Page {page} of {totalPages} ({total} total)
            </p>
            <div className="flex gap-1">
              <button
                onClick={() => setPage(p => Math.max(1, p - 1))}
                disabled={page <= 1}
                className="p-1.5 rounded-lg hover:bg-gray-100 disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
              <button
                onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                disabled={page >= totalPages}
                className="p-1.5 rounded-lg hover:bg-gray-100 disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Detail Modal */}
      {selectedLog && (
        <AuditDetailModal log={selectedLog} onClose={() => setSelectedLog(null)} />
      )}
    </div>
  );
}

function ActionBadge({ action }: { action: string }) {
  const colors: Record<string, string> = {
    create: 'bg-green-50 text-green-700',
    update: 'bg-blue-50 text-blue-700',
    delete: 'bg-red-50 text-red-700',
    revoke: 'bg-red-50 text-red-700',
    approve: 'bg-emerald-50 text-emerald-700',
    reject: 'bg-orange-50 text-orange-700',
    cancel: 'bg-yellow-50 text-yellow-700',
    login: 'bg-purple-50 text-purple-700',
    process: 'bg-indigo-50 text-indigo-700',
  };
  const cls = colors[action] || 'bg-gray-50 text-gray-700';
  return (
    <span className={`inline-flex px-2 py-0.5 text-xs font-medium rounded-full ${cls}`}>
      {action}
    </span>
  );
}

function AuditDetailModal({ log, onClose }: { log: AuditLog; onClose: () => void }) {
  return (
    <div className="fixed inset-0 bg-black/40 z-50 flex items-center justify-center p-4" onClick={onClose}>
      <div
        className="bg-white rounded-2xl max-w-2xl w-full max-h-[80vh] overflow-hidden flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between p-5 border-b border-gray-100">
          <div>
            <h2 className="text-lg font-semibold text-gray-900">Audit Log Detail</h2>
            <p className="text-sm text-gray-500">{formatTimestamp(log.created_at)}</p>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-gray-100 rounded-lg">
            <X className="w-5 h-5 text-gray-400" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-4">
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <p className="text-gray-500">User</p>
              <p className="font-medium">{log.user_full_name || '-'}</p>
              <p className="text-xs text-gray-400">{log.user_email}</p>
            </div>
            <div>
              <p className="text-gray-500">IP Address</p>
              <p className="font-medium">{log.ip_address || '-'}</p>
            </div>
            <div>
              <p className="text-gray-500">Entity Type</p>
              <p className="font-medium">{formatEntityType(log.entity_type)}</p>
            </div>
            <div>
              <p className="text-gray-500">Action</p>
              <ActionBadge action={log.action} />
            </div>
            <div className="col-span-2">
              <p className="text-gray-500">Entity ID</p>
              <p className="font-mono text-xs">{log.entity_id || '-'}</p>
            </div>
            <div className="col-span-2">
              <p className="text-gray-500">Description</p>
              <p className="font-medium">{log.description || '-'}</p>
            </div>
            <div className="col-span-2">
              <p className="text-gray-500">User Agent</p>
              <p className="text-xs text-gray-600 break-all">{log.user_agent || '-'}</p>
            </div>
          </div>

          {log.old_values && Object.keys(log.old_values).length > 0 && (
            <div>
              <h3 className="text-sm font-medium text-gray-700 mb-2">Previous Values</h3>
              <pre className="bg-red-50 text-red-900 rounded-lg p-3 text-xs overflow-x-auto max-h-48">
                {JSON.stringify(log.old_values, null, 2)}
              </pre>
            </div>
          )}

          {log.new_values && Object.keys(log.new_values).length > 0 && (
            <div>
              <h3 className="text-sm font-medium text-gray-700 mb-2">New Values</h3>
              <pre className="bg-green-50 text-green-900 rounded-lg p-3 text-xs overflow-x-auto max-h-48">
                {JSON.stringify(log.new_values, null, 2)}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
