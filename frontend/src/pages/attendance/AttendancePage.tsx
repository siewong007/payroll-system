import { useState, useEffect, useRef, useCallback } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  QrCode, RefreshCw, Clock, CheckCircle2,
  Filter, Plus, MapPin, Fingerprint,
  AlertCircle, Calendar, User, LogIn, LogOut, MoreVertical,
  ChevronLeft, ChevronRight, Pencil, AlertTriangle, Timer, Download,
  Link2, Copy, Trash2, ShieldCheck, X,
} from 'lucide-react';
import QRCode from 'qrcode';
import {
  getAttendanceMethod,
  getAttendanceRecords,
  generateQrToken,
  createManualAttendance,
  updateAttendanceRecord,
  downloadAttendanceCsv,
  type AttendanceRecordWithEmployee,
} from '@/api/attendance';
import {
  listKioskCredentials,
  createKioskCredential,
  revokeKioskCredential,
  type CreateKioskCredentialResponse,
} from '@/api/kiosk';
import { useAuth } from '@/context/AuthContext';
import { WorkScheduleCard } from '@/components/attendance/WorkScheduleCard';
import { GeofenceCard } from '@/components/attendance/GeofenceCard';

const STATUS_CONFIG = {
  present: { label: 'Present', color: 'bg-emerald-100 text-emerald-700' },
  late:    { label: 'Late',    color: 'bg-amber-100  text-amber-700'   },
  absent:  { label: 'Absent',  color: 'bg-red-100    text-red-700'     },
  half_day:{ label: 'Half Day',color: 'bg-blue-100   text-blue-700'    },
} as const;

const METHOD_CONFIG = {
  qr_code:  { label: 'QR Code',  icon: QrCode,      color: 'text-violet-600' },
  face_id:  { label: 'Face ID',  icon: Fingerprint,  color: 'text-sky-600'    },
  manual:   { label: 'Manual',   icon: User,         color: 'text-gray-500'   },
} as const;

function formatTime(iso: string | null) {
  if (!iso) return '—';
  return new Date(iso).toLocaleTimeString('en-MY', { hour: '2-digit', minute: '2-digit' });
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString('en-MY', { day: 'numeric', month: 'short', year: 'numeric' });
}

// ─── Kiosk Credentials Modal ────────────────────────────────────────────────

function formatDateTime(iso: string | null) {
  if (!iso) return '—';
  return new Date(iso).toLocaleString('en-MY', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function KioskCredentialsModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [label, setLabel] = useState('');
  const [created, setCreated] = useState<CreateKioskCredentialResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState('');

  const { data: credentials = [], isLoading } = useQuery({
    queryKey: ['kiosk-credentials'],
    queryFn: listKioskCredentials,
  });

  const createMut = useMutation({
    mutationFn: (l: string) => createKioskCredential(l),
    onSuccess: (data) => {
      setCreated(data);
      setLabel('');
      setError('');
      queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] });
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(e.response?.data?.error || 'Failed to create kiosk link');
    },
  });

  const revokeMut = useMutation({
    mutationFn: (id: string) => revokeKioskCredential(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] }),
  });

  const copyUrl = async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard write may fail in non-HTTPS contexts; user can copy manually.
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-2xl p-6 max-h-[90vh] overflow-y-auto">
        <div className="flex items-start justify-between mb-1">
          <div>
            <h3 className="font-semibold text-gray-900">Kiosk links</h3>
            <p className="text-sm text-gray-500">
              Each link opens the rotating QR on a tablet without anyone needing to log in.
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* One-time URL display after creation */}
        {created && (
          <div className="mt-5 border border-emerald-200 bg-emerald-50 rounded-xl p-4">
            <div className="flex items-start gap-2">
              <ShieldCheck className="w-5 h-5 text-emerald-600 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-emerald-900">
                  Copy this link now — it will not be shown again.
                </p>
                <p className="text-xs text-emerald-700 mt-0.5">
                  Anyone with the link can display the QR. Treat it like a password.
                </p>
                <div className="mt-3 flex items-center gap-2">
                  <input
                    readOnly
                    value={created.public_url}
                    onFocus={(e) => e.currentTarget.select()}
                    className="flex-1 px-3 py-2 bg-white border border-emerald-300 rounded-lg text-xs font-mono text-gray-800 outline-none truncate"
                  />
                  <button
                    onClick={() => copyUrl(created.public_url)}
                    className="flex items-center gap-1.5 px-3 py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium rounded-lg transition-colors"
                  >
                    <Copy className="w-3.5 h-3.5" />
                    {copied ? 'Copied' : 'Copy'}
                  </button>
                </div>
                <button
                  onClick={() => setCreated(null)}
                  className="mt-3 text-xs text-emerald-800 hover:underline"
                >
                  Done — hide this link
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Create form */}
        {!created && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!label.trim()) {
                setError('Please give this kiosk a name');
                return;
              }
              createMut.mutate(label.trim());
            }}
            className="mt-5 flex gap-2"
          >
            <input
              type="text"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="Reception tablet"
              maxLength={100}
              className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
            <button
              type="submit"
              disabled={createMut.isPending}
              className="flex items-center gap-1.5 px-4 py-2 bg-black hover:bg-gray-800 text-white text-sm font-medium rounded-lg transition-colors disabled:opacity-50"
            >
              <Plus className="w-4 h-4" />
              {createMut.isPending ? 'Creating…' : 'Create link'}
            </button>
          </form>
        )}
        {error && (
          <p className="mt-2 text-sm text-red-600 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5" /> {error}
          </p>
        )}

        {/* List */}
        <div className="mt-6">
          <h4 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">
            Existing links
          </h4>
          {isLoading ? (
            <div className="flex items-center justify-center h-24">
              <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-black" />
            </div>
          ) : credentials.length === 0 ? (
            <p className="text-sm text-gray-400 italic">No kiosk links yet.</p>
          ) : (
            <div className="border border-gray-200 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-gray-50">
                  <tr>
                    {['Label', 'Prefix', 'Created', 'Last used', 'Status', ''].map((h) => (
                      <th
                        key={h}
                        className="px-3 py-2 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider"
                      >
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-100">
                  {credentials.map((c) => {
                    const revoked = c.revoked_at != null;
                    return (
                      <tr key={c.id} className={revoked ? 'opacity-60' : ''}>
                        <td className="px-3 py-2.5 text-gray-900 font-medium">{c.label}</td>
                        <td className="px-3 py-2.5 text-xs font-mono text-gray-500">
                          {c.token_prefix}…
                        </td>
                        <td className="px-3 py-2.5 text-xs text-gray-500">
                          {formatDateTime(c.created_at)}
                        </td>
                        <td className="px-3 py-2.5 text-xs text-gray-500">
                          <div>{formatDateTime(c.last_used_at)}</div>
                          {c.last_used_ip && (
                            <div className="text-gray-400 font-mono">{c.last_used_ip}</div>
                          )}
                        </td>
                        <td className="px-3 py-2.5">
                          {revoked ? (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-500">
                              Revoked
                            </span>
                          ) : (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-700">
                              Active
                            </span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 text-right">
                          {!revoked && (
                            <button
                              onClick={() => {
                                if (confirm(`Revoke "${c.label}"? Tablets using this link will stop working within ~5 minutes.`)) {
                                  revokeMut.mutate(c.id);
                                }
                              }}
                              disabled={revokeMut.isPending}
                              className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 transition-colors disabled:opacity-50"
                              title="Revoke"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ─── QR Panel ────────────────────────────────────────────────────────────────

function QrPanel() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [timeLeft, setTimeLeft] = useState(0);
  const [isExpired, setIsExpired] = useState(false);
  const [showKiosks, setShowKiosks] = useState(false);
  const queryClient = useQueryClient();

  const { data: token, refetch: generateNew } = useQuery({
    queryKey: ['attendance-qr'],
    queryFn: generateQrToken,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });

  // Draw QR on canvas
  useEffect(() => {
    if (!token || !canvasRef.current) return;
    QRCode.toCanvas(canvasRef.current, token.scan_url, {
      width: 240,
      margin: 2,
      color: { dark: '#111827', light: '#ffffff' },
    });
  }, [token]);

  // Countdown timer
  useEffect(() => {
    if (!token) return;
    const tick = () => {
      const left = Math.max(0, Math.floor((new Date(token.expires_at).getTime() - Date.now()) / 1000));
      setTimeLeft(left);
      setIsExpired(left === 0);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [token]);

  const handleRefresh = useCallback(async () => {
    await queryClient.removeQueries({ queryKey: ['attendance-qr'] });
    generateNew();
  }, [queryClient, generateNew]);

  // Auto-refresh when expired
  useEffect(() => {
    if (isExpired) {
      const t = setTimeout(handleRefresh, 500);
      return () => clearTimeout(t);
    }
  }, [isExpired, handleRefresh]);

  const pct = token ? Math.max(0, (timeLeft / (token.ttl_seconds || 300)) * 100) : 0;

  return (
    <div className="bg-white rounded-2xl shadow p-6 flex flex-col items-center gap-4">
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center gap-2">
          <QrCode className="w-5 h-5 text-violet-600" />
          <h2 className="font-semibold text-gray-900">Attendance QR Code</h2>
        </div>
        <button
          onClick={handleRefresh}
          className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-gray-900 bg-gray-100 hover:bg-gray-200 px-3 py-1.5 rounded-lg transition-colors"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          Refresh
        </button>
      </div>

      {/* QR Canvas */}
      <div className={`relative rounded-2xl overflow-hidden p-3 transition-opacity ${isExpired ? 'opacity-30' : 'opacity-100'}`}>
        <canvas ref={canvasRef} className="block rounded-xl" />
        {isExpired && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="bg-white/90 rounded-xl px-4 py-2 text-sm font-semibold text-gray-700">
              Refreshing…
            </div>
          </div>
        )}
      </div>

      {/* Countdown */}
      <div className="w-full">
        <div className="flex justify-between text-xs text-gray-500 mb-1">
          <span>Expires in</span>
          <span className={`font-semibold tabular-nums ${timeLeft <= 10 ? 'text-red-500' : 'text-gray-700'}`}>
            {timeLeft}s
          </span>
        </div>
        <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
          <div
            className={`h-full rounded-full transition-all ${timeLeft <= 10 ? 'bg-red-400' : 'bg-violet-500'}`}
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      <p className="text-xs text-gray-400 text-center">
        Display on kiosk screen. Employees scan with their phone.
      </p>

      <button
        onClick={() => setShowKiosks(true)}
        className="w-full flex items-center justify-center gap-2 bg-violet-600 hover:bg-violet-700 text-white text-sm font-medium py-2.5 rounded-xl transition-colors"
      >
        <Link2 className="w-4 h-4" />
        Kiosk links
      </button>
      <a
        href="/attendance/kiosk"
        target="_blank"
        rel="noopener noreferrer"
        className="text-xs text-gray-400 hover:text-gray-600 underline"
      >
        Open kiosk in this browser (legacy, requires login)
      </a>

      {showKiosks && <KioskCredentialsModal onClose={() => setShowKiosks(false)} />}
    </div>
  );
}

// ─── Stats Bar ────────────────────────────────────────────────────────────────

function StatsBar({ records }: { records: AttendanceRecordWithEmployee[] }) {
  const today = new Date().toDateString();
  const todayRecords = records.filter(r => new Date(r.check_in_at).toDateString() === today);
  const present = todayRecords.filter(r => r.status === 'present').length;
  const late    = todayRecords.filter(r => r.status === 'late').length;
  const checkedOut = todayRecords.filter(r => r.check_out_at).length;

  return (
    <div className="grid grid-cols-3 gap-4 mb-6">
      {[
        { icon: CheckCircle2, label: 'Present Today', value: present, color: 'text-emerald-600', bg: 'bg-emerald-50' },
        { icon: Clock,        label: 'Late',          value: late,    color: 'text-amber-600',   bg: 'bg-amber-50'   },
        { icon: LogOut,       label: 'Checked Out',   value: checkedOut, color: 'text-blue-600', bg: 'bg-blue-50'    },
      ].map(s => (
        <div key={s.label} className="bg-white rounded-2xl shadow p-5 flex items-center gap-4">
          <div className={`${s.bg} ${s.color} p-3 rounded-xl`}>
            <s.icon className="w-5 h-5" />
          </div>
          <div>
            <p className="text-2xl font-bold text-gray-900">{s.value}</p>
            <p className="text-xs text-gray-500">{s.label}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Manual Entry Modal ───────────────────────────────────────────────────────

function ManualEntryModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    employee_id: '',
    check_in_at: new Date().toISOString().slice(0, 16),
    check_out_at: '',
    status: 'present',
    notes: '',
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createManualAttendance,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      onClose();
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(e.response?.data?.error || 'Failed to create record');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.employee_id.trim()) { setError('Employee ID is required'); return; }
    mutation.mutate({
      employee_id: form.employee_id.trim(),
      check_in_at: new Date(form.check_in_at).toISOString(),
      check_out_at: form.check_out_at ? new Date(form.check_out_at).toISOString() : undefined,
      status: form.status,
      notes: form.notes || undefined,
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-md p-6">
        <h3 className="font-semibold text-gray-900 mb-5">Manual Attendance Entry</h3>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Employee ID (UUID)</label>
            <input
              type="text"
              value={form.employee_id}
              onChange={e => setForm(p => ({ ...p, employee_id: e.target.value }))}
              placeholder="employee-uuid"
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Check In</label>
            <input type="datetime-local" value={form.check_in_at}
              onChange={e => setForm(p => ({ ...p, check_in_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Check Out (optional)</label>
            <input type="datetime-local" value={form.check_out_at}
              onChange={e => setForm(p => ({ ...p, check_out_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Status</label>
            <select value={form.status} onChange={e => setForm(p => ({ ...p, status: e.target.value as keyof typeof STATUS_CONFIG }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            >
              <option value="present">Present</option>
              <option value="late">Late</option>
              <option value="absent">Absent</option>
              <option value="half_day">Half Day</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Notes (optional)</label>
            <textarea value={form.notes} onChange={e => setForm(p => ({ ...p, notes: e.target.value }))}
              rows={2} className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none resize-none"
            />
          </div>
          {error && <p className="text-sm text-red-600 flex items-center gap-1"><AlertCircle className="w-3.5 h-3.5" />{error}</p>}
          <div className="flex gap-3 pt-2">
            <button type="button" onClick={onClose}
              className="flex-1 py-2 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 transition-colors">
              Cancel
            </button>
            <button type="submit" disabled={mutation.isPending}
              className="flex-1 py-2 bg-black text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50">
              {mutation.isPending ? 'Saving…' : 'Save'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ─── Edit Attendance Modal ────────────────────────────────────────────────────

function EditAttendanceModal({ record, onClose }: { record: AttendanceRecordWithEmployee; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    check_in_at: new Date(record.check_in_at).toISOString().slice(0, 16),
    check_out_at: record.check_out_at ? new Date(record.check_out_at).toISOString().slice(0, 16) : '',
    status: record.status,
    notes: record.notes ?? '',
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => updateAttendanceRecord(record.id, {
      check_in_at: new Date(form.check_in_at).toISOString(),
      check_out_at: form.check_out_at ? new Date(form.check_out_at).toISOString() : undefined,
      status: form.status,
      notes: form.notes || undefined,
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      onClose();
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(e.response?.data?.error || 'Failed to update record');
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-md p-6">
        <h3 className="font-semibold text-gray-900 mb-1">Edit Attendance</h3>
        <p className="text-sm text-gray-500 mb-5">{record.full_name} ({record.employee_number})</p>
        <form onSubmit={e => { e.preventDefault(); mutation.mutate(); }} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Check In</label>
            <input type="datetime-local" value={form.check_in_at}
              onChange={e => setForm(p => ({ ...p, check_in_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Check Out</label>
            <input type="datetime-local" value={form.check_out_at}
              onChange={e => setForm(p => ({ ...p, check_out_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Status</label>
            <select value={form.status} onChange={e => setForm(p => ({ ...p, status: e.target.value as keyof typeof STATUS_CONFIG }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none">
              <option value="present">Present</option>
              <option value="late">Late</option>
              <option value="absent">Absent</option>
              <option value="half_day">Half Day</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Notes</label>
            <textarea value={form.notes} onChange={e => setForm(p => ({ ...p, notes: e.target.value }))}
              rows={2} className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none resize-none"
            />
          </div>
          {error && <p className="text-sm text-red-600 flex items-center gap-1"><AlertCircle className="w-3.5 h-3.5" />{error}</p>}
          <div className="flex gap-3 pt-2">
            <button type="button" onClick={onClose}
              className="flex-1 py-2 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 transition-colors">
              Cancel
            </button>
            <button type="submit" disabled={mutation.isPending}
              className="flex-1 py-2 bg-black text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50">
              {mutation.isPending ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function AttendancePage() {
  const { user } = useAuth();
  const [filters, setFilters] = useState({ date_from: '', date_to: '', status: '', method: '' });
  const [page, setPage] = useState(1);
  const [showManual, setShowManual] = useState(false);
  const [editRecord, setEditRecord] = useState<AttendanceRecordWithEmployee | null>(null);
  const [exporting, setExporting] = useState(false);
  const perPage = 50;

  const { data: method } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
  });

  const { data: result, isLoading } = useQuery({
    queryKey: ['attendance-records', filters, page],
    queryFn: () => getAttendanceRecords({
      date_from: filters.date_from || undefined,
      date_to:   filters.date_to   || undefined,
      status:    filters.status    || undefined,
      method:    filters.method    || undefined,
      page,
      per_page:  perPage,
    }),
  });

  const records = result?.data ?? [];
  const totalPages = result?.total_pages ?? 1;
  const total = result?.total ?? 0;

  const isAdmin = ['admin', 'super_admin', 'hr_manager', 'payroll_admin', 'exec'].includes(user?.role ?? '');

  // Reset to page 1 when filters change
  const updateFilter = (key: string, value: string) => {
    setFilters(p => ({ ...p, [key]: value }));
    setPage(1);
  };

  return (
    <div>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">Attendance</h1>
          {method && (
            <p className="text-sm text-gray-500 mt-0.5">
              Method:&nbsp;
              <span className="font-medium text-gray-700">
                {method.method === 'qr_code' ? 'QR Code' : 'Face ID'}
              </span>
              {method.is_company_override && (
                <span className="ml-1.5 text-xs bg-amber-100 text-amber-700 px-1.5 py-0.5 rounded">Company Override</span>
              )}
            </p>
          )}
        </div>
        <div className="flex gap-2">
          {isAdmin && (
            <>
              <button
                onClick={async () => {
                  setExporting(true);
                  try {
                    await downloadAttendanceCsv({
                      date_from: filters.date_from || undefined,
                      date_to:   filters.date_to   || undefined,
                      status:    filters.status    || undefined,
                    });
                  } finally {
                    setExporting(false);
                  }
                }}
                disabled={exporting}
                className="flex items-center gap-2 text-sm font-medium px-4 py-2 rounded-xl border border-gray-300 hover:bg-gray-50 transition-colors disabled:opacity-50"
              >
                <Download className="w-4 h-4" />
                {exporting ? 'Exporting…' : 'Export CSV'}
              </button>
              <button
                onClick={() => setShowManual(true)}
                className="flex items-center gap-2 bg-black text-white text-sm font-medium px-4 py-2 rounded-xl hover:bg-gray-800 transition-colors"
              >
                <Plus className="w-4 h-4" />
                Manual Entry
              </button>
            </>
          )}
        </div>
      </div>

      <StatsBar records={records} />

      {/* Work Schedule & Geofence (admin only) */}
      {isAdmin && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <WorkScheduleCard />
          <GeofenceCard />
        </div>
      )}

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-6">
        {/* QR Panel — only in QR mode */}
        {method?.method === 'qr_code' && isAdmin && (
          <div className="xl:col-span-1">
            <QrPanel />
          </div>
        )}

        {/* Records Table */}
        <div className={method?.method === 'qr_code' && isAdmin ? 'xl:col-span-3' : 'xl:col-span-4'}>
          <div className="bg-white rounded-2xl shadow">
            {/* Filters */}
            <div className="flex flex-wrap items-center gap-3 p-5 border-b border-gray-100">
              <div className="flex items-center gap-2 text-sm text-gray-600">
                <Filter className="w-4 h-4" /> Filters:
              </div>
              <input
                type="date"
                value={filters.date_from}
                onChange={e => updateFilter('date_from', e.target.value)}
                className="px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
                placeholder="From"
              />
              <input
                type="date"
                value={filters.date_to}
                onChange={e => updateFilter('date_to', e.target.value)}
                className="px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
                placeholder="To"
              />
              <select
                value={filters.status}
                onChange={e => updateFilter('status', e.target.value)}
                className="px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
              >
                <option value="">All Status</option>
                <option value="present">Present</option>
                <option value="late">Late</option>
                <option value="absent">Absent</option>
                <option value="half_day">Half Day</option>
              </select>
              <select
                value={filters.method}
                onChange={e => updateFilter('method', e.target.value)}
                className="px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
              >
                <option value="">All Methods</option>
                <option value="qr_code">QR Code</option>
                <option value="face_id">Face ID</option>
                <option value="manual">Manual</option>
              </select>
              {(filters.date_from || filters.date_to || filters.status || filters.method) && (
                <button
                  onClick={() => { setFilters({ date_from: '', date_to: '', status: '', method: '' }); setPage(1); }}
                  className="text-xs text-gray-500 hover:text-gray-900 underline"
                >
                  Clear
                </button>
              )}
            </div>

            {/* Table */}
            {isLoading ? (
              <div className="flex items-center justify-center h-48">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
              </div>
            ) : records.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-48 text-gray-400">
                <Calendar className="w-10 h-10 mb-2 opacity-40" />
                <p className="text-sm">No attendance records found</p>
              </div>
            ) : (
              <>
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-gray-100">
                        {['Date', 'Employee', 'Check In', 'Check Out', 'Hours', 'Method', 'Status', 'Location', ''].map(h => (
                          <th key={h} className="px-5 py-3 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider whitespace-nowrap">
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-gray-50">
                      {records.map(r => {
                        const statusCfg = STATUS_CONFIG[r.status as keyof typeof STATUS_CONFIG];
                        const methodCfg = METHOD_CONFIG[r.method as keyof typeof METHOD_CONFIG];
                        const MethodIcon = methodCfg?.icon ?? MoreVertical;
                        return (
                          <tr key={r.id} className="hover:bg-gray-50/50 transition-colors">
                            <td className="px-5 py-3.5 whitespace-nowrap text-gray-600">
                              {formatDate(r.check_in_at)}
                            </td>
                            <td className="px-5 py-3.5">
                              <div className="font-medium text-gray-900">{r.full_name}</div>
                              <div className="text-xs text-gray-400">{r.employee_number}</div>
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700">
                              <div className="flex items-center gap-1.5">
                                <LogIn className="w-3.5 h-3.5 text-emerald-500" />
                                {formatTime(r.check_in_at)}
                              </div>
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700">
                              <div className="flex items-center gap-1.5">
                                <LogOut className="w-3.5 h-3.5 text-gray-400" />
                                {formatTime(r.check_out_at)}
                              </div>
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700">
                              {r.hours_worked != null ? (
                                <div className="text-xs">
                                  <span>{Number(r.hours_worked).toFixed(1)}h</span>
                                  {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
                                    <span className="ml-1 text-amber-600 font-medium">
                                      <Timer className="w-3 h-3 inline" /> +{Number(r.overtime_hours).toFixed(1)}
                                    </span>
                                  )}
                                </div>
                              ) : (
                                <span className="text-gray-300 text-xs">---</span>
                              )}
                            </td>
                            <td className="px-5 py-3.5">
                              <div className={`flex items-center gap-1.5 ${methodCfg?.color ?? ''}`}>
                                <MethodIcon className="w-3.5 h-3.5" />
                                <span className="text-xs font-medium">{methodCfg?.label ?? r.method}</span>
                              </div>
                            </td>
                            <td className="px-5 py-3.5">
                              <div className="flex items-center gap-1.5">
                                {statusCfg && (
                                  <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg.color}`}>
                                    {statusCfg.label}
                                  </span>
                                )}
                                {r.is_outside_geofence && (
                                  <span title="Checked in outside office radius" className="text-amber-500">
                                    <AlertTriangle className="w-3.5 h-3.5" />
                                  </span>
                                )}
                              </div>
                            </td>
                            <td className="px-5 py-3.5">
                              {r.latitude && r.longitude ? (
                                <a
                                  href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  className="flex items-center gap-1 text-sky-600 hover:text-sky-700 text-xs"
                                >
                                  <MapPin className="w-3.5 h-3.5" />
                                  View
                                </a>
                              ) : (
                                <span className="text-gray-300 text-xs">---</span>
                              )}
                            </td>
                            {isAdmin && (
                              <td className="px-3 py-3.5">
                                <button
                                  onClick={() => setEditRecord(r)}
                                  className="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100 transition-colors"
                                  title="Edit record"
                                >
                                  <Pencil className="w-3.5 h-3.5" />
                                </button>
                              </td>
                            )}
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                {/* Pagination */}
                {totalPages > 1 && (
                  <div className="flex items-center justify-between px-5 py-3 border-t border-gray-100">
                    <p className="text-xs text-gray-500">
                      Showing {(page - 1) * perPage + 1}–{Math.min(page * perPage, total)} of {total} records
                    </p>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => setPage(p => Math.max(1, p - 1))}
                        disabled={page <= 1}
                        className="p-1.5 rounded-lg hover:bg-gray-100 disabled:opacity-30 disabled:cursor-not-allowed"
                      >
                        <ChevronLeft className="w-4 h-4" />
                      </button>
                      {Array.from({ length: Math.min(totalPages, 5) }, (_, i) => {
                        let p: number;
                        if (totalPages <= 5) p = i + 1;
                        else if (page <= 3) p = i + 1;
                        else if (page >= totalPages - 2) p = totalPages - 4 + i;
                        else p = page - 2 + i;
                        return (
                          <button
                            key={p}
                            onClick={() => setPage(p)}
                            className={`w-8 h-8 rounded-lg text-xs font-medium transition-colors ${
                              p === page ? 'bg-black text-white' : 'hover:bg-gray-100 text-gray-600'
                            }`}
                          >
                            {p}
                          </button>
                        );
                      })}
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
              </>
            )}
          </div>
        </div>
      </div>

      {showManual && <ManualEntryModal onClose={() => setShowManual(false)} />}
      {editRecord && <EditAttendanceModal record={editRecord} onClose={() => setEditRecord(null)} />}
    </div>
  );
}
