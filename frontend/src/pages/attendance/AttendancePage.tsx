import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient, keepPreviousData } from '@tanstack/react-query';
import {
  QrCode, RefreshCw, Clock, CheckCircle2,
  Filter, Plus, MapPin, Fingerprint,
  AlertCircle, Calendar, User, LogIn, LogOut, MoreVertical,
  ChevronLeft, ChevronRight, Pencil, AlertTriangle, Timer, Download,
  Link2, Copy, Trash2, ShieldCheck, X, ListChecks, Eye, EyeOff,
} from 'lucide-react';
import QRCode from 'qrcode';
import {
  getAttendanceMethod,
  getAttendanceRecords,
  generateQrToken,
  createManualAttendance,
  updateAttendanceRecord,
  downloadAttendanceCsv,
  getAttendanceSummary,
  runAbsentMarking,
  setCompanyAttendanceMethod,
  type AttendanceRecordWithEmployee,
  type AttendanceSummaryItem,
} from '@/api/attendance';
import {
  listKioskCredentials,
  createKioskCredential,
  revokeKioskCredential,
  type CreateKioskCredentialResponse,
} from '@/api/kiosk';
import { useAuth } from '@/context/AuthContext';
import { hasAnyRole } from '@/lib/roles';
import { toDateTimeLocalValue, getErrorMessage } from '@/lib/utils';
import i18n from '@/i18n';
import {
  formatTime as formatTimeL,
  formatDateMedium,
  formatDateTime as formatDateTimeL,
} from '@/lib/format';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { WorkScheduleCard } from '@/components/attendance/WorkScheduleCard';
import { GeofenceCard } from '@/components/attendance/GeofenceCard';
import { NetworkCard } from '@/components/attendance/NetworkCard';

/**
 * Today's date on the *company* calendar, not the viewer's. Attendance days
 * are bucketed in the company timezone server-side, so a viewer in another
 * zone (or near midnight) would otherwise label the wrong day "today".
 */
function todayInZone(timeZone: string | undefined): string {
  try {
    // i18n-ok: en-CA produces ISO YYYY-MM-DD for API params, not display.
    return new Intl.DateTimeFormat('en-CA', {
      timeZone: timeZone || 'Asia/Kuala_Lumpur',
      year: 'numeric', month: '2-digit', day: '2-digit',
    }).format(new Date());
  } catch {
    return new Intl.DateTimeFormat('en-CA').format(new Date()); // i18n-ok
  }
}

function firstOfMonth(day: string): string {
  return `${day.slice(0, 7)}-01`;
}

const STATUS_COLORS = {
  present: 'bg-emerald-100 text-emerald-700',
  late:    'bg-amber-100  text-amber-700',
  absent:  'bg-red-100    text-red-700',
  half_day:'bg-blue-100   text-blue-700',
} as const;

const METHOD_CONFIG = {
  qr_code:  { icon: QrCode,      color: 'text-violet-600' },
  face_id:  { icon: Fingerprint, color: 'text-sky-600'    },
  manual:   { icon: User,        color: 'text-gray-500'   },
} as const;

function attStatusLabel(status: string): string {
  return i18n.t(`enums.attendanceStatus.${status}`, { defaultValue: status });
}

function attMethodLabel(method: string): string {
  return i18n.t(`enums.attendanceMethod.${method}`, { defaultValue: method });
}

function formatTime(iso: string | null) {
  return iso ? formatTimeL(iso) : '—';
}

function formatDate(iso: string) {
  return formatDateMedium(iso);
}

// ─── Kiosk Credentials Modal ────────────────────────────────────────────────

function formatDateTime(iso: string | null) {
  return iso ? formatDateTimeL(iso) : '—';
}

function KioskCredentialsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [label, setLabel] = useState('');
  const [created, setCreated] = useState<CreateKioskCredentialResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState('');

  const { data: credentials = [], isLoading, isError: listError } = useQuery({
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
      setError(getErrorMessage(e, t('attendance.kioskModal.createFailed')));
    },
  });

  const revokeMut = useMutation({
    mutationFn: (id: string) => revokeKioskCredential(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] }),
    onError: (e) => setError(getErrorMessage(e, t('attendance.kioskModal.revokeFailed'))),
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
            <h3 className="font-semibold text-gray-900">{t('attendance.kioskModal.title')}</h3>
            <p className="text-sm text-gray-500">
              {t('attendance.kioskModal.body')}
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
                  {t('attendance.kioskModal.copyNow')}
                </p>
                <p className="text-xs text-emerald-700 mt-0.5">
                  {t('attendance.kioskModal.treatLikePassword')}
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
                    {copied ? t('common.copied') : t('common.copy')}
                  </button>
                </div>
                <button
                  onClick={() => setCreated(null)}
                  className="mt-3 text-xs text-emerald-800 hover:underline"
                >
                  {t('attendance.kioskModal.doneHide')}
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
                setError(t('attendance.kioskModal.nameRequired'));
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
              placeholder={t('attendance.kioskModal.namePlaceholder')}
              maxLength={100}
              className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
            <button
              type="submit"
              disabled={createMut.isPending}
              className="flex items-center gap-1.5 px-4 py-2 bg-black hover:bg-gray-800 text-white text-sm font-medium rounded-lg transition-colors disabled:opacity-50"
            >
              <Plus className="w-4 h-4" />
              {createMut.isPending ? t('common.creating') : t('attendance.kioskModal.createLink')}
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
            {t('attendance.kioskModal.existing')}
          </h4>
          {isLoading ? (
            <div className="flex items-center justify-center h-24">
              <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-black" />
            </div>
          ) : listError ? (
            // Must not read as "no links yet" — a 403 or network failure is a
            // different situation entirely.
            <p className="text-sm text-red-600 flex items-center gap-1.5">
              <AlertCircle className="w-3.5 h-3.5" />
              {t('attendance.kioskModal.loadFailed')}
            </p>
          ) : credentials.length === 0 ? (
            <p className="text-sm text-gray-400 italic">{t('attendance.kioskModal.empty')}</p>
          ) : (
            <div className="border border-gray-200 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-gray-50">
                  <tr>
                    {[t('attendance.kioskModal.cols.label'), t('attendance.kioskModal.cols.prefix'), t('attendance.kioskModal.cols.created'), t('attendance.kioskModal.cols.lastUsed'), t('common.status'), ''].map((h) => (
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
                              {t('attendance.kioskModal.revoked')}
                            </span>
                          ) : (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-700">
                              {t('common.active')}
                            </span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 text-right">
                          {!revoked && (
                            <button
                              onClick={() => {
                                if (confirm(t('attendance.kioskModal.revokeConfirm', { label: c.label }))) {
                                  revokeMut.mutate(c.id);
                                }
                              }}
                              disabled={revokeMut.isPending}
                              className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 transition-colors disabled:opacity-50"
                              title={t('attendance.kioskModal.revoke')}
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

function QrPanel({ canGenerate }: { canGenerate: boolean }) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [timeLeft, setTimeLeft] = useState(0);
  const [isExpired, setIsExpired] = useState(false);
  const [showKiosks, setShowKiosks] = useState(false);
  // Opt-in display. The panel used to mint a token every 5 minutes for as long
  // as any admin tab sat open — even one only viewing records.
  const [showQr, setShowQr] = useState(false);
  const queryClient = useQueryClient();

  const { data: token, dataUpdatedAt, refetch: generateNew, isError } = useQuery({
    queryKey: ['attendance-qr'],
    queryFn: generateQrToken,
    enabled: showQr && canGenerate,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });

  // Draw QR on canvas. `showQr` is a dependency because the canvas is
  // unmounted while hidden: re-showing within the TTL returns the same cached
  // token object, so keying on `token` alone left the fresh canvas unpainted
  // until the token expired.
  useEffect(() => {
    if (!showQr || !token || !canvasRef.current) return;
    QRCode.toCanvas(canvasRef.current, token.scan_url, {
      width: 240,
      margin: 2,
      color: { dark: '#111827', light: '#ffffff' },
    });
  }, [token, showQr]);

  // Countdown timer, measured from the TTL against the moment the response
  // arrived here. `expires_at` is a *server* instant, so comparing it with
  // Date.now() folded the device's clock offset into the countdown: a machine a
  // few minutes fast read every fresh token as already expired and re-minted
  // immediately. `dataUpdatedAt` is stamped locally, so both terms share a clock
  // and the offset cancels.
  useEffect(() => {
    if (!token || !dataUpdatedAt) return;
    const ttl = token.ttl_seconds > 0 ? token.ttl_seconds : 300;
    const tick = () => {
      const left = Math.max(0, Math.ceil(ttl - (Date.now() - dataUpdatedAt) / 1000));
      setTimeLeft(left);
      setIsExpired(left === 0);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [token, dataUpdatedAt]);

  const handleRefresh = useCallback(async () => {
    await queryClient.removeQueries({ queryKey: ['attendance-qr'] });
    generateNew();
  }, [queryClient, generateNew]);

  // Auto-refresh when expired, but only while the panel is actually visible on
  // screen. A backgrounded tab kept minting tokens indefinitely.
  useEffect(() => {
    if (!isExpired || !showQr) return;
    if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
      const onVisible = () => {
        if (document.visibilityState === 'visible') void handleRefresh();
      };
      document.addEventListener('visibilitychange', onVisible);
      return () => document.removeEventListener('visibilitychange', onVisible);
    }
    const t = setTimeout(handleRefresh, 500);
    return () => clearTimeout(t);
  }, [isExpired, showQr, handleRefresh]);

  const pct = token ? Math.max(0, (timeLeft / (token.ttl_seconds || 300)) * 100) : 0;
  const ttlMinutes = token ? Math.round((token.ttl_seconds || 300) / 60) : 5;

  return (
    <div className="bg-white rounded-2xl shadow p-6 flex flex-col items-center gap-4">
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center gap-2">
          <QrCode className="w-5 h-5 text-violet-600" />
          <h2 className="font-semibold text-gray-900">{t('attendance.qrPanel.title')}</h2>
        </div>
        {showQr && canGenerate && (
          <button
            onClick={handleRefresh}
            className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-gray-900 bg-gray-100 hover:bg-gray-200 px-3 py-1.5 rounded-lg transition-colors"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            {t('common.refresh')}
          </button>
        )}
      </div>

      {!canGenerate ? (
        <p className="text-sm text-gray-500 text-center py-4">
          {t('attendance.qrPanel.noPermission')}
        </p>
      ) : !showQr ? (
        <>
          <div className="w-full py-6 flex flex-col items-center gap-2 text-center">
            <div className="w-14 h-14 rounded-2xl bg-violet-50 flex items-center justify-center">
              <QrCode className="w-7 h-7 text-violet-500" />
            </div>
            <p className="text-sm text-gray-500 px-2">
              {t('attendance.qrPanel.showHint')}
            </p>
          </div>
          <button
            onClick={() => setShowQr(true)}
            className="w-full flex items-center justify-center gap-2 bg-black hover:bg-gray-800 text-white text-sm font-medium py-2.5 rounded-xl transition-colors"
          >
            <Eye className="w-4 h-4" />
            {t('attendance.qrPanel.showButton')}
          </button>
        </>
      ) : isError ? (
        <div className="w-full py-6 flex flex-col items-center gap-3 text-center">
          <AlertCircle className="w-8 h-8 text-red-400" />
          <p className="text-sm text-gray-600">{t('attendance.qrPanel.generateFailed')}</p>
          <button
            onClick={handleRefresh}
            className="flex items-center gap-1.5 text-sm font-medium text-gray-900 hover:underline"
          >
            <RefreshCw className="w-3.5 h-3.5" /> {t('common.retry')}
          </button>
        </div>
      ) : (
        <>
          {/* QR Canvas */}
          <div className={`relative rounded-2xl overflow-hidden p-3 transition-opacity ${isExpired ? 'opacity-30' : 'opacity-100'}`}>
            <canvas ref={canvasRef} className="block rounded-xl" />
            {isExpired && (
              <div className="absolute inset-0 flex items-center justify-center">
                <div className="bg-white/90 rounded-xl px-4 py-2 text-sm font-semibold text-gray-700">
                  {t('attendance.qrPanel.refreshing')}
                </div>
              </div>
            )}
          </div>

          {/* Countdown */}
          <div className="w-full">
            <div className="flex justify-between text-xs text-gray-500 mb-1">
              <span>{t('attendance.qrPanel.expiresIn')}</span>
              <span className={`font-semibold tabular-nums ${timeLeft <= 10 ? 'text-red-500' : 'text-gray-700'}`}>
                {t('attendance.kiosk.secondsShort', { count: timeLeft })}
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
            {t('attendance.qrPanel.rotateNote', { minutes: ttlMinutes })}
          </p>

          <button
            onClick={() => setShowQr(false)}
            className="text-xs text-gray-400 hover:text-gray-600 flex items-center gap-1"
          >
            <EyeOff className="w-3 h-3" /> {t('attendance.qrPanel.hide')}
          </button>
        </>
      )}

      <button
        onClick={() => setShowKiosks(true)}
        className="w-full flex items-center justify-center gap-2 bg-violet-600 hover:bg-violet-700 text-white text-sm font-medium py-2.5 rounded-xl transition-colors"
      >
        <Link2 className="w-4 h-4" />
        {t('attendance.kioskModal.title')}
      </button>

      {showKiosks && <KioskCredentialsModal onClose={() => setShowKiosks(false)} />}
    </div>
  );
}

// ─── Stats Bar ────────────────────────────────────────────────────────────────

function StatsBar({ today, onShowOpen }: { today: string; onShowOpen: () => void }) {
  const { t } = useTranslation();
  // These tiles report today across the whole company, so they must not be
  // derived from `records` — that is one 50-row page of the *currently filtered*
  // query, so a date filter or a second page made the numbers undercount or
  // collapse to zero while still being labelled "today".
  // Key is prefixed with 'attendance-records' so the existing invalidation after
  // a manual entry or edit refreshes these tiles too.
  const { data, isError } = useQuery({
    queryKey: ['attendance-records', 'today-stats', today],
    queryFn: () => getAttendanceSummary({ date_from: today, date_to: today }),
  });
  const items = data ?? [];
  const present = items.reduce((sum, i) => sum + i.present_days, 0);
  const late = items.reduce((sum, i) => sum + i.late_days, 0);
  const stillIn = items.reduce((sum, i) => sum + i.unchecked_out_days, 0);

  if (isError) {
    return (
      <div className="bg-white rounded-2xl shadow p-5 mb-6 flex items-center gap-3 text-sm text-gray-600">
        <AlertCircle className="w-5 h-5 text-red-400 shrink-0" />
        {t('attendance.stats.loadFailed')}
      </div>
    );
  }

  const tiles = [
    { icon: CheckCircle2, label: t('attendance.stats.presentToday'), short: t('attendance.stats.present'), value: present, color: 'text-emerald-600', bg: 'bg-emerald-50', action: undefined },
    { icon: Clock,        label: t('attendance.stats.late'),         short: t('attendance.stats.late'),    value: late,    color: 'text-amber-600',   bg: 'bg-amber-50',   action: undefined },
    // Actionable: these are the sessions someone has to chase or correct.
    { icon: LogOut,       label: t('attendance.stats.stillIn'),      short: t('attendance.stats.stillInShort'), value: stillIn, color: 'text-blue-600', bg: 'bg-blue-50', action: onShowOpen },
  ];

  return (
    <div className="grid grid-cols-3 gap-2.5 sm:gap-4 mb-6">
      {tiles.map(s => {
        // Phone tiles stack icon over value: three side-by-side rows at 375px
        // pushed the label clean off the right edge of the card.
        const body = (
          <>
            <div className={`${s.bg} ${s.color} p-2 sm:p-3 rounded-xl shrink-0 w-fit`}>
              <s.icon className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
            <div className="min-w-0">
              <p className="text-xl sm:text-2xl font-bold text-gray-900 leading-tight">{s.value}</p>
              <p className="text-[11px] sm:text-xs text-gray-500 leading-tight">
                <span className="sm:hidden">{s.short}</span>
                <span className="hidden sm:inline">{s.label}</span>
                {s.action && s.value > 0 && <span className="ml-1 text-blue-600 font-medium">{t('attendance.stats.review')}</span>}
              </p>
            </div>
          </>
        );
        const shell =
          'bg-white rounded-2xl shadow p-3 sm:p-5 flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 min-w-0';

        return s.action ? (
          <button
            key={s.label}
            type="button"
            onClick={s.action}
            className={`${shell} text-left cursor-pointer hover:shadow-md transition-shadow`}
          >
            {body}
          </button>
        ) : (
          <div key={s.label} className={shell}>{body}</div>
        );
      })}
    </div>
  );
}

// ─── Summary Tab ──────────────────────────────────────────────────────────────

/**
 * One employee's period aggregate as a card, for the same reason as
 * `RecordCard`: nine numeric columns do not survive a phone. The four day
 * counts stay on one row so they remain scannable down the list.
 */
function SummaryCard({ item: i }: { item: AttendanceSummaryItem }) {
  const { t } = useTranslation();
  const counts = [
    { label: t('attendance.stats.present'), value: i.present_days, color: 'text-emerald-700' },
    { label: t('attendance.stats.late'),    value: i.late_days,    color: 'text-amber-700'   },
    { label: attStatusLabel('absent'),      value: i.absent_days,  color: 'text-red-700'     },
    { label: t('attendance.stats.half'),    value: i.half_days,    color: 'text-blue-700'    },
  ];

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-medium text-gray-900 line-clamp-2">{i.full_name}</p>
          <p className="text-xs text-gray-400 truncate">
            {i.employee_number}{i.department ? ` · ${i.department}` : ''}
          </p>
        </div>
        {i.unchecked_out_days > 0 && (
          <span
            title={t('attendance.summary.noCheckout')}
            className="shrink-0 inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-0.5 text-xs font-medium text-amber-700"
          >
            <AlertTriangle className="w-3.5 h-3.5" />
            {t('attendance.summary.openDays', { count: i.unchecked_out_days })}
          </span>
        )}
      </div>

      <div className="mt-3 grid grid-cols-4 gap-2">
        {counts.map(c => (
          <div key={c.label} className="rounded-lg bg-gray-50 px-2 py-1.5 text-center">
            <p className={`text-base font-semibold tabular-nums ${c.color}`}>{c.value}</p>
            <p className="text-[11px] text-gray-500">{c.label}</p>
          </div>
        ))}
      </div>

      <div className="mt-2.5 flex items-center gap-4 text-xs text-gray-600">
        <span className="tabular-nums">{t('attendance.summary.hoursWorked', { value: Number(i.total_hours).toFixed(1) })}</span>
        <span className="tabular-nums">
          <Timer className="w-3 h-3 inline text-amber-600" /> {t('attendance.summary.hoursOt', { value: Number(i.overtime_hours).toFixed(1) })}
        </span>
      </div>
    </div>
  );
}

/**
 * Per-employee aggregate over a date range — the view HR actually needs each
 * payroll cycle. The endpoint always supported the range and filters; nothing
 * consumed them until now (the tiles above only ever asked for "today").
 */
function SummaryTab({ today, canExport }: { today: string; canExport: boolean }) {
  const { t } = useTranslation();
  const [range, setRange] = useState({ date_from: firstOfMonth(today), date_to: today });
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState('');

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['attendance-summary', range],
    queryFn: () => getAttendanceSummary(range),
    placeholderData: keepPreviousData,
  });
  const items = data ?? [];

  return (
    <div className="bg-white rounded-2xl shadow">
      <div className="flex flex-wrap items-center gap-2.5 sm:gap-3 p-4 sm:p-5 border-b border-gray-100">
        <div className="flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600">
          <Calendar className="w-4 h-4" /> {t('attendance.summary.period')}
        </div>
        <input
          type="date"
          value={range.date_from}
          onChange={e => setRange(p => ({ ...p, date_from: e.target.value }))}
          className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
        <input
          type="date"
          value={range.date_to}
          onChange={e => setRange(p => ({ ...p, date_to: e.target.value }))}
          className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
        {canExport && (
          <button
            onClick={async () => {
              setExporting(true);
              setExportError('');
              try {
                await downloadAttendanceCsv({ date_from: range.date_from, date_to: range.date_to });
              } catch (e) {
                setExportError(getErrorMessage(e, t('attendance.page.exportFailed')));
              } finally {
                setExporting(false);
              }
            }}
            disabled={exporting}
            className="w-full sm:w-auto sm:ml-auto flex items-center justify-center gap-2 whitespace-nowrap text-sm font-medium px-4 py-2 sm:py-1.5 rounded-lg border border-gray-300 hover:bg-gray-50 transition-colors disabled:opacity-50"
          >
            <Download className="w-4 h-4 shrink-0" />
            {exporting ? t('common.exporting') : t('attendance.summary.exportPeriod')}
          </button>
        )}
      </div>

      {exportError && (
        <p className="px-5 py-2 text-sm text-red-600 flex items-center gap-1.5">
          <AlertCircle className="w-3.5 h-3.5" /> {exportError}
        </p>
      )}

      {isLoading ? (
        <div className="flex items-center justify-center h-48">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
        </div>
      ) : isError ? (
        <div className="flex flex-col items-center justify-center h-48 gap-2 text-gray-600">
          <AlertCircle className="w-8 h-8 text-red-400" />
          <p className="text-sm">{t('attendance.summary.loadFailed')}</p>
          <button onClick={() => refetch()} className="text-sm font-medium text-gray-900 hover:underline">
            {t('common.retry')}
          </button>
        </div>
      ) : (
        <>
        {/* Cards below lg, table above — see SummaryCard. */}
        <div className="divide-y divide-gray-100 lg:hidden">
          {items.map(i => (
            <SummaryCard key={i.employee_id} item={i} />
          ))}
        </div>

        <div className="hidden lg:block overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-100">
                {[t('common.employee'), t('common.department'), t('attendance.stats.present'), t('attendance.stats.late'), attStatusLabel('absent'), t('attendance.stats.half'), t('attendance.summary.hours'), t('attendance.summary.overtime'), t('attendance.summary.open')].map(h => (
                  <th key={h} className="px-5 py-3 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider whitespace-nowrap">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-50">
              {items.map(i => (
                <tr key={i.employee_id} className="hover:bg-gray-50/50 transition-colors">
                  <td className="px-5 py-3">
                    <div className="font-medium text-gray-900">{i.full_name}</div>
                    <div className="text-xs text-gray-400">{i.employee_number}</div>
                  </td>
                  <td className="px-5 py-3 text-gray-600">{i.department || t('common.unassigned')}</td>
                  <td className="px-5 py-3 tabular-nums text-emerald-700">{i.present_days}</td>
                  <td className="px-5 py-3 tabular-nums text-amber-700">{i.late_days}</td>
                  <td className="px-5 py-3 tabular-nums text-red-700">{i.absent_days}</td>
                  <td className="px-5 py-3 tabular-nums text-blue-700">{i.half_days}</td>
                  <td className="px-5 py-3 tabular-nums text-gray-700">{t('attendance.hoursShort', { value: Number(i.total_hours).toFixed(1) })}</td>
                  <td className="px-5 py-3 tabular-nums text-gray-700">{t('attendance.hoursShort', { value: Number(i.overtime_hours).toFixed(1) })}</td>
                  <td className="px-5 py-3 tabular-nums">
                    {i.unchecked_out_days > 0 ? (
                      <span className="inline-flex items-center gap-1 text-amber-600 font-medium">
                        <AlertTriangle className="w-3.5 h-3.5" />
                        {i.unchecked_out_days}
                      </span>
                    ) : (
                      <span className="text-gray-300">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Outside the table wrapper — that wrapper is display:none on phones,
            which would have turned "no results" into a blank panel. */}
        {items.length === 0 && (
          <div className="flex flex-col items-center justify-center h-40 text-gray-400">
            <Calendar className="w-10 h-10 mb-2 opacity-40" />
            <p className="text-sm">{t('attendance.summary.empty')}</p>
          </div>
        )}
        </>
      )}
    </div>
  );
}

// ─── Manual Entry Modal ───────────────────────────────────────────────────────

function ManualEntryModal({
  onClose,
  presetEmployee,
}: {
  onClose: () => void;
  presetEmployee?: { id: string; label: string };
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    employee_id: presetEmployee?.id ?? '',
    check_in_at: toDateTimeLocalValue(new Date()),
    check_out_at: '',
    status: 'present',
    notes: '',
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createManualAttendance,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.manual.createFailed'))),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.employee_id.trim()) { setError(t('attendance.manual.employeeRequired')); return; }
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
        <h3 className="font-semibold text-gray-900 mb-5">{t('attendance.manual.title')}</h3>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.employee')}</label>
            {presetEmployee ? (
              <div className="px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm text-gray-800">
                {presetEmployee.label}
              </div>
            ) : (
              <EmployeePicker
                value={form.employee_id}
                onChange={(id) => setForm(p => ({ ...p, employee_id: id }))}
              />
            )}
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.manual.checkIn')}</label>
            <input type="datetime-local" value={form.check_in_at}
              onChange={e => setForm(p => ({ ...p, check_in_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.manual.checkOutOptional')}</label>
            <input type="datetime-local" value={form.check_out_at}
              onChange={e => setForm(p => ({ ...p, check_out_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.status')}</label>
            <select value={form.status} onChange={e => setForm(p => ({ ...p, status: e.target.value as keyof typeof STATUS_COLORS }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            >
              <option value="present">{attStatusLabel('present')}</option>
              <option value="late">{attStatusLabel('late')}</option>
              <option value="absent">{attStatusLabel('absent')}</option>
              <option value="half_day">{attStatusLabel('half_day')}</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.manual.notesOptional')}</label>
            <textarea value={form.notes} onChange={e => setForm(p => ({ ...p, notes: e.target.value }))}
              rows={2} className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none resize-none"
            />
          </div>
          {error && <p className="text-sm text-red-600 flex items-center gap-1"><AlertCircle className="w-3.5 h-3.5" />{error}</p>}
          <div className="flex gap-3 pt-2">
            <button type="button" onClick={onClose}
              className="flex-1 py-2 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 transition-colors">
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={mutation.isPending}
              className="flex-1 py-2 bg-black text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50">
              {mutation.isPending ? t('common.saving') : t('common.save')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ─── Edit Attendance Modal ────────────────────────────────────────────────────

function EditAttendanceModal({ record, onClose }: { record: AttendanceRecordWithEmployee; onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    check_in_at: toDateTimeLocalValue(record.check_in_at),
    check_out_at: record.check_out_at ? toDateTimeLocalValue(record.check_out_at) : '',
    status: record.status,
    notes: record.notes ?? '',
    reason: '',
  });
  const [error, setError] = useState('');

  const hadCheckOut = record.check_out_at != null;
  const hadNotes = (record.notes ?? '') !== '';

  const mutation = useMutation({
    mutationFn: () => updateAttendanceRecord(record.id, {
      check_in_at: new Date(form.check_in_at).toISOString(),
      check_out_at: form.check_out_at ? new Date(form.check_out_at).toISOString() : undefined,
      status: form.status,
      notes: form.notes || undefined,
      // Emptying a field previously did nothing: the server read a missing
      // value as "keep existing", so the record silently kept its old
      // check-out (and its payroll-relevant hours). Say "clear" explicitly.
      clear_check_out: hadCheckOut && !form.check_out_at ? true : undefined,
      clear_notes: hadNotes && !form.notes ? true : undefined,
      reason: form.reason.trim(),
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.edit.updateFailed'))),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.reason.trim()) {
      setError(t('attendance.edit.reasonRequired'));
      return;
    }
    mutation.mutate();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-md p-6 max-h-[90vh] overflow-y-auto">
        <h3 className="font-semibold text-gray-900 mb-1">{t('attendance.edit.title')}</h3>
        <p className="text-sm text-gray-500 mb-5">{record.full_name} ({record.employee_number})</p>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.manual.checkIn')}</label>
            <input type="datetime-local" value={form.check_in_at}
              onChange={e => setForm(p => ({ ...p, check_in_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('attendance.manual.checkOut')}</label>
            <input type="datetime-local" value={form.check_out_at}
              onChange={e => setForm(p => ({ ...p, check_out_at: e.target.value }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
            {hadCheckOut && !form.check_out_at && (
              <p className="mt-1 text-xs text-amber-600 flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                {t('attendance.edit.reopenWarning')}
              </p>
            )}
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.status')}</label>
            <select value={form.status} onChange={e => setForm(p => ({ ...p, status: e.target.value as keyof typeof STATUS_COLORS }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none">
              <option value="present">{attStatusLabel('present')}</option>
              <option value="late">{attStatusLabel('late')}</option>
              <option value="absent">{attStatusLabel('absent')}</option>
              <option value="half_day">{attStatusLabel('half_day')}</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.notes')}</label>
            <textarea value={form.notes} onChange={e => setForm(p => ({ ...p, notes: e.target.value }))}
              rows={2} className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none resize-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              {t('attendance.edit.reason')} <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={form.reason}
              onChange={e => setForm(p => ({ ...p, reason: e.target.value }))}
              placeholder={t('attendance.edit.reasonPlaceholder')}
              maxLength={400}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
            />
            <p className="mt-1 text-xs text-gray-400">
              {t('attendance.edit.reasonNote')}
            </p>
          </div>
          {error && <p className="text-sm text-red-600 flex items-center gap-1"><AlertCircle className="w-3.5 h-3.5" />{error}</p>}
          <div className="flex gap-3 pt-2">
            <button type="button" onClick={onClose}
              className="flex-1 py-2 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 transition-colors">
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={mutation.isPending}
              className="flex-1 py-2 bg-black text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50">
              {mutation.isPending ? t('common.saving') : t('common.saveChanges')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ─── Record Card (narrow screens) ─────────────────────────────────────────────

/**
 * One attendance record as a card. The nine-column table needs ~1000px, so on
 * anything narrower it became a horizontal scroller where Status, Location and
 * the edit control sat off-screen — invisible unless you thought to swipe the
 * table sideways. Same fields, stacked.
 */
function RecordCard({
  record: r,
  canEdit,
  onEdit,
}: {
  record: AttendanceRecordWithEmployee;
  canEdit: boolean;
  onEdit: () => void;
}) {
  const { t } = useTranslation();
  const statusCfg = STATUS_COLORS[r.status as keyof typeof STATUS_COLORS];
  const methodCfg = METHOD_CONFIG[r.method as keyof typeof METHOD_CONFIG];
  const MethodIcon = methodCfg?.icon ?? MoreVertical;

  return (
    <div className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          {/* Two lines, not `truncate` — this is the field you identify the
              record by, and Malaysian full names routinely exceed one line. */}
          <p className="font-medium text-gray-900 line-clamp-2">{r.full_name}</p>
          <p className="text-xs text-gray-400 truncate">
            {r.employee_number} · {formatDate(r.check_in_at)}
          </p>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          {statusCfg && (
            <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg}`}>
              {attStatusLabel(r.status)}
            </span>
          )}
          {r.is_outside_geofence && (
            <span title={t('attendance.records.outsideGeofence')} className="text-amber-500">
              <AlertTriangle className="w-4 h-4" />
            </span>
          )}
          {canEdit && (
            <button
              onClick={onEdit}
              className="-mr-1 p-2 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100 transition-colors"
              title={t('attendance.records.editRecord')}
            >
              <Pencil className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-3">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">{t('attendance.records.checkIn')}</p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700">
            <LogIn className="w-3.5 h-3.5 shrink-0 text-emerald-500" />
            {formatTime(r.check_in_at)}
          </p>
        </div>
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">{t('attendance.records.checkOut')}</p>
          <p className="mt-0.5 flex items-center gap-1.5 text-sm tabular-nums text-gray-700">
            <LogOut className="w-3.5 h-3.5 shrink-0 text-gray-400" />
            {formatTime(r.check_out_at)}
          </p>
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs">
        <span className="tabular-nums text-gray-600">
          {r.hours_worked != null ? t('attendance.hoursShort', { value: Number(r.hours_worked).toFixed(1) }) : '—'}
          {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
            <span className="ml-1 text-amber-600 font-medium">
              <Timer className="w-3 h-3 inline" /> {t('attendance.hoursOtShort', { value: Number(r.overtime_hours).toFixed(1) })}
            </span>
          )}
        </span>
        <span className={`flex items-center gap-1.5 ${methodCfg?.color ?? 'text-gray-500'}`}>
          <MethodIcon className="w-3.5 h-3.5" />
          <span className="font-medium">{methodCfg ? attMethodLabel(r.method) : r.method}</span>
        </span>
        {r.latitude && r.longitude && (
          <a
            href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 text-sky-600 hover:text-sky-700"
          >
            <MapPin className="w-3.5 h-3.5" />
            {t('attendance.records.location')}
          </a>
        )}
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

/** Empty filter set. Named so "clear" and the triage shortcuts can't drift from
 *  the initial state as filters are added. */
const NO_FILTERS = {
  date_from: '', date_to: '', status: '', method: '', employee_id: '',
  open_only: false, outside_geofence_only: false,
};

export function AttendancePage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'records' | 'summary'>('records');
  const [filters, setFilters] = useState(NO_FILTERS);
  const [page, setPage] = useState(1);
  const [showManual, setShowManual] = useState(false);
  const [editRecord, setEditRecord] = useState<AttendanceRecordWithEmployee | null>(null);
  const [exporting, setExporting] = useState(false);
  const [pageError, setPageError] = useState('');
  const [backfillDate, setBackfillDate] = useState('');
  const perPage = 50;

  const { data: method } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
  });

  const today = todayInZone(method?.timezone);

  const { data: result, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['attendance-records', filters, page],
    queryFn: () => getAttendanceRecords({
      date_from:   filters.date_from   || undefined,
      date_to:     filters.date_to     || undefined,
      status:      filters.status      || undefined,
      method:      filters.method      || undefined,
      employee_id: filters.employee_id || undefined,
      open_only:   filters.open_only   || undefined,
      outside_geofence_only: filters.outside_geofence_only || undefined,
      page,
      per_page:  perPage,
    }),
    // Keep the previous page rendered while the next one loads instead of
    // collapsing the table into a spinner on every click.
    placeholderData: keepPreviousData,
  });

  const records = result?.data ?? [];
  const totalPages = result?.total_pages ?? 1;
  const total = result?.total ?? 0;
  const forbidden = (error as { response?: { status?: number } } | null)?.response?.status === 403;

  // Capability sets mirroring the backend gates, so we never render a control
  // whose endpoint will 403. Previously one broad `isAdmin` showed payroll_admin
  // and exec buttons that always failed, and hid Export from finance which the
  // backend allows.
  const canView = hasAnyRole(user, ['super_admin', 'admin', 'hr_manager', 'payroll_admin', 'finance', 'exec']);
  const canEdit = hasAnyRole(user, ['super_admin', 'admin', 'hr_manager']);
  const canManageKiosks = hasAnyRole(user, ['super_admin', 'admin', 'hr_manager', 'payroll_admin']);
  const canGenerateQr = canManageKiosks;
  const canConfigure = hasAnyRole(user, ['super_admin', 'admin', 'hr_manager']);

  // The settings page tells super admins that company admins can switch method
  // here when override is enabled. Until now nothing called the endpoint, so
  // that documented flow was impossible.
  const canSwitchMethod =
    Boolean(method?.allow_company_override) && hasAnyRole(user, ['super_admin', 'admin']);

  const methodMut = useMutation({
    mutationFn: (m: string) => setCompanyAttendanceMethod(m),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      setPageError('');
    },
    onError: (e) => setPageError(getErrorMessage(e, t('attendance.page.methodFailed'))),
  });

  const backfillMut = useMutation({
    mutationFn: (date: string) => runAbsentMarking(date),
    onSuccess: (res) => {
      queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      setPageError('');
      setBackfillDate('');
      alert(t('attendance.page.backfillDone', { count: res.marked, date: res.date }));
    },
    onError: (e) => setPageError(getErrorMessage(e, t('attendance.page.backfillFailed'))),
  });

  // Reset to page 1 when filters change
  const updateFilter = (key: string, value: string | boolean) => {
    setFilters(p => ({ ...p, [key]: value }));
    setPage(1);
  };

  const hasFilters = useMemo(
    () => Boolean(
      filters.date_from || filters.date_to || filters.status ||
      filters.method || filters.employee_id || filters.open_only ||
      filters.outside_geofence_only
    ),
    [filters],
  );

  const showOpenSessions = () => {
    setTab('records');
    setFilters({ ...NO_FILTERS, open_only: true });
    setPage(1);
  };

  return (
    <div>
      {/* Header — actions drop to their own row on phones rather than
          squeezing the title into a two-line button stack. */}
      <div className="flex flex-col gap-3 mb-6 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-2xl font-bold text-gray-900">{t('attendance.page.title')}</h1>
          {method && (
            <div className="text-sm text-gray-500 mt-0.5 flex items-center gap-1.5">
              <span>{t('attendance.page.method')}</span>
              {canSwitchMethod ? (
                <select
                  value={method.method}
                  onChange={e => methodMut.mutate(e.target.value)}
                  disabled={methodMut.isPending}
                  className="px-2 py-0.5 border border-gray-200 rounded-lg text-sm font-medium text-gray-700 outline-none focus:ring-1 focus:ring-black disabled:opacity-50"
                >
                  <option value="qr_code">{attMethodLabel('qr_code')}</option>
                  <option value="face_id">{attMethodLabel('face_id')}</option>
                </select>
              ) : (
                <span className="font-medium text-gray-700">
                  {attMethodLabel(method.method)}
                </span>
              )}
              {method.is_company_override && (
                <span className="text-xs bg-amber-100 text-amber-700 px-1.5 py-0.5 rounded">{t('attendance.page.companyOverride')}</span>
              )}
            </div>
          )}
        </div>
        <div className="flex gap-2 shrink-0">
          {canView && (
            <button
              onClick={async () => {
                setExporting(true);
                setPageError('');
                try {
                  await downloadAttendanceCsv({
                    date_from:   filters.date_from   || undefined,
                    date_to:     filters.date_to     || undefined,
                    status:      filters.status      || undefined,
                    method:      filters.method      || undefined,
                    employee_id: filters.employee_id || undefined,
                  });
                } catch (e) {
                  // Previously try/finally with no catch: a failed export was an
                  // unhandled rejection and the admin saw nothing at all.
                  setPageError(getErrorMessage(e, t('attendance.page.exportFailed')));
                } finally {
                  setExporting(false);
                }
              }}
              disabled={exporting}
              className="flex flex-1 sm:flex-none items-center justify-center gap-2 whitespace-nowrap text-sm font-medium px-4 py-2 rounded-xl border border-gray-300 hover:bg-gray-50 transition-colors disabled:opacity-50"
            >
              <Download className="w-4 h-4 shrink-0" />
              {exporting ? t('common.exporting') : t('attendance.page.exportCsv')}
            </button>
          )}
          {canEdit && (
            <button
              onClick={() => setShowManual(true)}
              className="flex flex-1 sm:flex-none items-center justify-center gap-2 whitespace-nowrap bg-black text-white text-sm font-medium px-4 py-2 rounded-xl hover:bg-gray-800 transition-colors"
            >
              <Plus className="w-4 h-4 shrink-0" />
              {t('attendance.page.manualEntry')}
            </button>
          )}
        </div>
      </div>

      {pageError && (
        <div className="mb-4 px-4 py-3 rounded-xl bg-red-50 border border-red-200 text-sm text-red-700 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {pageError}
        </div>
      )}

      <StatsBar today={today} onShowOpen={showOpenSessions} />

      {/* Work Schedule, Geofence & Office Network */}
      {canConfigure && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <WorkScheduleCard />
          <GeofenceCard />
          <NetworkCard />
        </div>
      )}

      {/* Tabs */}
      <div className="flex items-center gap-1 mb-4 border-b border-gray-200">
        {([
          { key: 'records', label: t('attendance.page.tabs.records'), icon: Clock },
          { key: 'summary', label: t('attendance.page.tabs.summary'), icon: ListChecks },
        ] as const).map(t => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium border-b-2 -mb-px transition-colors ${
              tab === t.key
                ? 'border-black text-gray-900'
                : 'border-transparent text-gray-500 hover:text-gray-800'
            }`}
          >
            <t.icon className="w-4 h-4" />
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'summary' ? (
        <SummaryTab today={today} canExport={canView} />
      ) : (
      <div className="grid grid-cols-1 xl:grid-cols-4 gap-6">
        {/* QR Panel — only in QR mode */}
        {method?.method === 'qr_code' && canManageKiosks && (
          <div className="xl:col-span-1">
            <QrPanel canGenerate={canGenerateQr} />
          </div>
        )}

        {/* Records Table */}
        <div className={method?.method === 'qr_code' && canManageKiosks ? 'xl:col-span-3' : 'xl:col-span-4'}>
          <div className="bg-white rounded-2xl shadow">
            {/* Filters */}
            <div className="flex flex-wrap items-center gap-2.5 sm:gap-3 p-4 sm:p-5 border-b border-gray-100">
              <div className="flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600">
                <Filter className="w-4 h-4" /> {t('attendance.page.filters')}
              </div>
              <input
                type="date"
                value={filters.date_from}
                onChange={e => updateFilter('date_from', e.target.value)}
                className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
                placeholder={t('attendance.page.from')}
              />
              <input
                type="date"
                value={filters.date_to}
                onChange={e => updateFilter('date_to', e.target.value)}
                className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
                placeholder={t('attendance.page.to')}
              />
              <select
                value={filters.status}
                onChange={e => updateFilter('status', e.target.value)}
                className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
              >
                <option value="">{t('attendance.page.allStatus')}</option>
                <option value="present">{attStatusLabel('present')}</option>
                <option value="late">{attStatusLabel('late')}</option>
                <option value="absent">{attStatusLabel('absent')}</option>
                <option value="half_day">{attStatusLabel('half_day')}</option>
              </select>
              <select
                value={filters.method}
                onChange={e => updateFilter('method', e.target.value)}
                className="min-w-[8.5rem] flex-1 sm:flex-none px-3 py-1.5 border border-gray-200 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
              >
                <option value="">{t('attendance.page.allMethods')}</option>
                <option value="qr_code">{attMethodLabel('qr_code')}</option>
                <option value="face_id">{attMethodLabel('face_id')}</option>
                <option value="manual">{attMethodLabel('manual')}</option>
              </select>
              <label className="flex w-full sm:w-auto items-center gap-1.5 text-sm text-gray-600 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={filters.open_only}
                  onChange={e => updateFilter('open_only', e.target.checked)}
                  className="rounded border-gray-300"
                />
                {t('attendance.page.openOnly')}
              </label>
              {/* The off-site flag was stored and shown per row, but with no
                  predicate the only way to find these was to page the whole
                  list hunting for the warning icon. */}
              <label className="flex w-full sm:w-auto items-center gap-1.5 text-sm text-gray-600 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={filters.outside_geofence_only}
                  onChange={e => updateFilter('outside_geofence_only', e.target.checked)}
                  className="rounded border-gray-300"
                />
                {t('attendance.page.offsiteOnly')}
              </label>
              {hasFilters && (
                <button
                  onClick={() => {
                    setFilters(NO_FILTERS);
                    setPage(1);
                  }}
                  className="text-xs text-gray-500 hover:text-gray-900 underline"
                >
                  {t('common.clear')}
                </button>
              )}
              {/* Employee filter — the API always accepted employee_id; without
                  a control, auditing one person meant paging the whole company. */}
              <div className="w-full sm:w-72">
                <EmployeePicker
                  value={filters.employee_id}
                  onChange={(id) => updateFilter('employee_id', id)}
                  placeholder={t('attendance.page.filterEmployee')}
                />
              </div>
            </div>

            {/* Backfill a day the daily absence job missed (e.g. deploy window) */}
            {canEdit && (
              <div className="flex flex-wrap items-center gap-2 px-4 sm:px-5 py-3 border-b border-gray-100 bg-gray-50/60">
                <span className="text-xs text-gray-500">
                  {t('attendance.page.backfillHint')}
                </span>
                <input
                  type="date"
                  value={backfillDate}
                  max={today}
                  onChange={e => setBackfillDate(e.target.value)}
                  className="px-2.5 py-1 border border-gray-200 rounded-lg text-xs outline-none focus:ring-1 focus:ring-black"
                />
                <button
                  onClick={() => backfillDate && backfillMut.mutate(backfillDate)}
                  disabled={!backfillDate || backfillMut.isPending}
                  className="text-xs font-medium px-3 py-1 rounded-lg border border-gray-300 bg-white hover:bg-gray-100 transition-colors disabled:opacity-40"
                >
                  {backfillMut.isPending ? t('common.running') : t('attendance.page.markAbsences')}
                </button>
                <span className="text-xs text-gray-400">
                  {t('attendance.page.backfillSafe')}
                </span>
              </div>
            )}

            {/* Table */}
            {isLoading ? (
              <div className="flex items-center justify-center h-48">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
              </div>
            ) : isError ? (
              // A 403 or a network failure must not render as "no records" —
              // that told an unauthorized user their company had no attendance.
              <div className="flex flex-col items-center justify-center h-48 gap-2 text-gray-600 px-6 text-center">
                <AlertCircle className="w-8 h-8 text-red-400" />
                <p className="text-sm">
                  {forbidden
                    ? t('attendance.page.forbidden')
                    : t('attendance.page.loadFailed')}
                </p>
                {!forbidden && (
                  <button onClick={() => refetch()} className="text-sm font-medium text-gray-900 hover:underline">
                    {t('common.retry')}
                  </button>
                )}
              </div>
            ) : records.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-48 text-gray-400">
                <Calendar className="w-10 h-10 mb-2 opacity-40" />
                <p className="text-sm">
                  {filters.open_only
                    ? t('attendance.page.emptyOpen')
                    : filters.outside_geofence_only
                      ? t('attendance.page.emptyOffsite')
                      : t('attendance.page.empty')}
                </p>
              </div>
            ) : (
              <>
                {/* Cards below lg, table above — see RecordCard. */}
                <div className="divide-y divide-gray-100 lg:hidden">
                  {records.map(r => (
                    <RecordCard
                      key={r.id}
                      record={r}
                      canEdit={canEdit}
                      onEdit={() => setEditRecord(r)}
                    />
                  ))}
                </div>

                <div className="hidden lg:block overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-gray-100">
                        {[t('common.date'), t('common.employee'), t('attendance.records.checkIn'), t('attendance.records.checkOut'), t('attendance.summary.hours'), t('attendance.records.method'), t('common.status'), t('attendance.records.location'), ''].map(h => (
                          <th key={h} className="px-5 py-3 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider whitespace-nowrap">
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-gray-50">
                      {records.map(r => {
                        const statusCfg = STATUS_COLORS[r.status as keyof typeof STATUS_COLORS];
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
                                  <span>{t('attendance.hoursShort', { value: Number(r.hours_worked).toFixed(1) })}</span>
                                  {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
                                    <span className="ml-1 text-amber-600 font-medium">
                                      <Timer className="w-3 h-3 inline" /> {t('attendance.hoursOtShort', { value: Number(r.overtime_hours).toFixed(1) })}
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
                                <span className="text-xs font-medium">{methodCfg ? attMethodLabel(r.method) : r.method}</span>
                              </div>
                            </td>
                            <td className="px-5 py-3.5">
                              <div className="flex items-center gap-1.5">
                                {statusCfg && (
                                  <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg}`}>
                                    {attStatusLabel(r.status)}
                                  </span>
                                )}
                                {r.is_outside_geofence && (
                                  <span title={t('attendance.records.outsideGeofence')} className="text-amber-500">
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
                                  {t('common.view')}
                                </a>
                              ) : (
                                <span className="text-gray-300 text-xs">---</span>
                              )}
                            </td>
                            {canEdit && (
                              <td className="px-3 py-3.5">
                                <button
                                  onClick={() => setEditRecord(r)}
                                  className="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 hover:bg-gray-100 transition-colors"
                                  title={t('attendance.records.editRecord')}
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
                  <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 px-4 sm:px-5 py-3 border-t border-gray-100">
                    <p className="text-xs text-gray-500">
                      {t('common.showing', { from: (page - 1) * perPage + 1, to: Math.min(page * perPage, total), total })}
                    </p>
                    <div className="flex items-center justify-center gap-1">
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
      )}

      {showManual && <ManualEntryModal onClose={() => setShowManual(false)} />}
      {editRecord && <EditAttendanceModal record={editRecord} onClose={() => setEditRecord(null)} />}
    </div>
  );
}
