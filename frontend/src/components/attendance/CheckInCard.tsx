import { useCallback, useEffect, useRef, useState } from 'react';
import { Link, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle, CheckCircle2, Fingerprint, LogOut, MapPin, QrCode, RefreshCw, ScanLine, TriangleAlert, Wifi, X,
} from 'lucide-react';
import {
  beginFaceIdCheckIn, checkInFaceId, checkInQr, checkOut, getAttendanceMethod, getMyTodayAttendance,
  type AttendanceMethodResponse, type AttendanceRecord,
} from '@/api/attendance';
import { getPasskeyCredential } from '@/lib/webauthn';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage } from '@/lib/utils';
import {
  buzz, classifyCheckInError, deriveTodayState, formatDuration, formatScheduleTime, formatZonedClock,
  formatZonedDateLine, formatZonedDateTime, formatZonedTime, getGeolocationResult, isPriorLocalDay,
  parseScheduleTime, shiftStanding, zonedParts, type GeolocationResult, type TodayState,
} from '@/lib/attendance';
import { QrScanSheet } from './QrScanSheet';

const FALLBACK_TZ = 'Asia/Kuala_Lumpur';

type Feedback = { tone: 'success' | 'error' | 'info'; text: string; href?: string; hrefLabel?: string };

/**
 * The employee's whole attendance interaction, in one tap.
 *
 * Mounted on the portal home so checking in costs a single tap from a cold
 * open. It previously took three — More → Attendance → Scan — and then handed
 * off to a separate `/attendance/scan` route to actually submit. Both the QR
 * and Face ID paths now resolve here without a navigation, so the in-memory
 * access token survives and the attendance-method fetch is served from cache.
 */
export function CheckInCard() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const location = useLocation();
  const queryClient = useQueryClient();

  const [scanning, setScanning] = useState(false);
  const [confirmingOut, setConfirmingOut] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [scanErrorText, setScanErrorText] = useState('');

  /**
   * A position resolved while the camera was already open. Fetching it here
   * rather than after the decode removes an up-to-8-second stall between
   * "code recognised" and "checked in".
   */
  const warmCoordsRef = useRef<Promise<GeolocationResult> | null>(null);
  const lastGeoErrorRef = useRef<number | null>(null);

  const hasEmployee = Boolean(user?.employee_id);
  const hasCompany = Boolean(user?.company_id);

  const { data: method } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
    enabled: hasCompany && hasEmployee,
  });
  const { data: today, isError: todayFailed, isLoading: todayLoading, refetch: refetchToday } = useQuery({
    queryKey: ['attendance-today'],
    queryFn: getMyTodayAttendance,
    enabled: hasEmployee,
    refetchInterval: 60_000,
  });

  const tz = method?.timezone || FALLBACK_TZ;
  const needsLocation = (method?.geofence_mode ?? 'none') !== 'none';
  const showWifiHint = method?.network_mode === 'enforce' || method?.network_mode === 'warn';
  const record = today?.record ?? null;
  const state = deriveTodayState(record);
  const historyHref = location.pathname.startsWith('/portal') ? '/portal/attendance' : '/my/attendance';
  const profileSecurityHref = location.pathname.startsWith('/portal') ? '/portal/profile?section=security' : '/account';

  useEffect(() => {
    if (!feedback || feedback.tone === 'error') return;
    const id = setTimeout(() => setFeedback(null), 6000);
    return () => clearTimeout(id);
  }, [feedback]);

  useEffect(() => {
    if (!confirmingOut) return;
    const id = setTimeout(() => setConfirmingOut(false), 5000);
    return () => clearTimeout(id);
  }, [confirmingOut]);

  const refreshAttendance = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
    void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
  }, [queryClient]);

  const succeed = useCallback((text: string) => {
    buzz([18, 40, 18]);
    setFeedback({ tone: 'success', text });
    refreshAttendance();
  }, [refreshAttendance]);

  /** Turn a server message into the recovery the employee can actually act on. */
  const explain = useCallback((err: unknown, fallback: string): Feedback => {
    // classify on the raw server prose — the translated text cannot match the
    // English phrase patterns — then show the localized version.
    const raw =
      typeof err === 'object' && err !== null && 'response' in err
        ? ((err as { response?: { data?: { error?: string } } }).response?.data?.error ?? '')
        : '';
    const message = getErrorMessage(err, fallback);
    switch (classifyCheckInError(raw || message)) {
      case 'already-checked-in':
        refreshAttendance();
        return { tone: 'info', text: t('attendance.checkin.alreadyIn') };
      case 'stale-session':
        return { tone: 'error', text: message, href: historyHref, hrefLabel: t('attendance.checkin.viewHistory') };
      case 'outside-geofence':
        return { tone: 'error', text: message };
      case 'offsite-network':
        // Nothing the employee can do beyond joining the office Wi-Fi, and the
        // server's message already says so — but a phone that has silently
        // fallen back to mobile data looks identical to one that is connected,
        // so name that explicitly.
        return {
          tone: 'error',
          text: t('attendance.checkin.offsiteSuffix', { message }),
        };
      case 'location-permission':
        if (lastGeoErrorRef.current === 2 || lastGeoErrorRef.current === 3) {
          return { tone: 'error', text: t('attendance.checkin.locationTimeout') };
        }
        return { tone: 'error', text: t('attendance.checkin.locationRequired') };
      case 'no-passkey':
        return { tone: 'error', text: message, href: profileSecurityHref, hrefLabel: t('attendance.checkin.addPasskey') };
      default:
        return { tone: 'error', text: message };
    }
  }, [historyHref, profileSecurityHref, refreshAttendance, t]);

  const resolveCoords = useCallback(async (allowWarm: boolean): Promise<GeolocationCoordinates | null> => {
    if (allowWarm && warmCoordsRef.current) {
      const warm = await warmCoordsRef.current;
      lastGeoErrorRef.current = warm.errorCode;
      if (warm.coords) return warm.coords;
      // Warm fix failed or timed out while the sheet opened — retry fresh.
      warmCoordsRef.current = null;
    }
    const fresh = await getGeolocationResult(needsLocation);
    lastGeoErrorRef.current = fresh.errorCode;
    return fresh.coords;
  }, [needsLocation]);

  const faceIdMut = useMutation({
    mutationFn: async () => {
      // The ceremony must stay inside the tap's user activation, so the GPS
      // prompt waits until after it rather than racing the biometric sheet.
      const { challenge_id, options } = await beginFaceIdCheckIn();
      const credential = await getPasskeyCredential(options.publicKey);
      const coords = await resolveCoords(false);
      return checkInFaceId(challenge_id, credential, coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => succeed(t('attendance.checkin.checkedInAt', { time: formatZonedTime(rec.check_in_at, tz) })),
    onError: (err) => {
      buzz(120);
      setFeedback(explain(err, t('attendance.checkin.faceIdFailed')));
    },
  });

  const qrMut = useMutation({
    mutationFn: async (token: string) => {
      const coords = await resolveCoords(true);
      return checkInQr(token, coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => {
      setScanning(false);
      setScanErrorText('');
      succeed(t('attendance.checkin.checkedInAt', { time: formatZonedTime(rec.check_in_at, tz) }));
    },
    onError: (err) => {
      buzz(120);
      const detail = explain(err, t('attendance.scan.checkInFailed'));
      // "Already checked in" means attendance IS recorded — close the camera
      // rather than leaving them pointing it at a code that cannot help.
      if (detail.tone === 'info') {
        setScanning(false);
        setScanErrorText('');
        setFeedback(detail);
      } else {
        setScanErrorText(detail.text);
      }
    },
  });

  const checkOutMut = useMutation({
    mutationFn: async () => {
      const coords = await resolveCoords(false);
      return checkOut(coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => {
      setConfirmingOut(false);
      buzz([18, 40, 18]);
      setFeedback({ tone: 'success', text: t('attendance.checkin.checkedOutAt', { time: formatZonedTime(rec.check_out_at, tz) }) });
      refreshAttendance();
    },
    onError: (err) => {
      setConfirmingOut(false);
      buzz(120);
      setFeedback(explain(err, t('attendance.checkin.checkOutFailed')));
    },
  });

  const submitting = faceIdMut.isPending || qrMut.isPending || checkOutMut.isPending;

  const handleCheckIn = () => {
    setFeedback(null);
    lastGeoErrorRef.current = null;
    // Waiting for the method to load before branching. Reading `method` while
    // it is still undefined used to open the QR camera at a Face ID company.
    if (!method) return;
    if (method.method === 'face_id') {
      faceIdMut.mutate();
    } else {
      warmCoordsRef.current = null;
      setScanErrorText('');
      setScanning(true);
    }
  };

  // ── Blocking states ────────────────────────────────────────────────────────

  if (user && !user.employee_id) {
    return (
      <Shell>
        <div className="flex items-start gap-3">
          <AlertCircle className="w-5 h-5 text-amber-300 shrink-0 mt-0.5" />
          <div>
            <h2 className="font-semibold">{t('attendance.checkin.notSetUpTitle')}</h2>
            <p className="text-sm text-white/75 mt-1">
              {t('attendance.checkin.notSetUpBody')}
            </p>
          </div>
        </div>
      </Shell>
    );
  }

  // Never offer a check-in button on a failed fetch: showing "Not checked in"
  // when we don't know invites a second attempt that fails as a duplicate.
  if (todayFailed) {
    return (
      <Shell>
        <div className="flex items-start gap-3 mb-4">
          <AlertCircle className="w-5 h-5 text-amber-300 shrink-0 mt-0.5" />
          <div>
            <h2 className="font-semibold">{t('attendance.checkin.loadFailedTitle')}</h2>
            <p className="text-sm text-white/75 mt-1">
              {t('attendance.checkin.loadFailedBody')}
            </p>
          </div>
        </div>
        <button onClick={() => void refetchToday()} className="w-full min-h-14 rounded-2xl bg-white text-teal-900 font-semibold flex items-center justify-center gap-2 active:scale-[0.98] transition-transform">
          <RefreshCw className="w-4 h-4" /> {t('common.retry')}
        </button>
      </Shell>
    );
  }

  return (
    <>
      <Shell>
        <CheckInClockDisplay
          state={state}
          record={record}
          tz={tz}
          method={method}
        />

        {/* Absent notice */}
        {state === 'absent' && (
          <div className="mt-4 flex items-start gap-2 rounded-2xl bg-amber-400/15 border border-amber-300/30 px-3 py-2.5 text-sm text-amber-100">
            <TriangleAlert className="w-4 h-4 mt-0.5 shrink-0" />
            <span>{t('attendance.checkin.absentNotice')}</span>
          </div>
        )}

        {/* Inline persistent error/info feedback on the card */}
        {feedback && feedback.tone === 'error' && (
          <div
            role="alert"
            className="mt-4 flex items-start gap-2.5 rounded-2xl bg-red-950/60 border border-red-300/30 px-3.5 py-3 text-sm text-red-100"
          >
            <AlertCircle className="w-4 h-4 mt-0.5 shrink-0 text-red-300" />
            <div className="flex-1 min-w-0">
              <p>{feedback.text}</p>
              {feedback.href && (
                <Link
                  to={feedback.href}
                  className="inline-block mt-1 font-semibold text-white underline underline-offset-2"
                >
                  {feedback.hrefLabel}
                </Link>
              )}
            </div>
            <button
              type="button"
              onClick={() => setFeedback(null)}
              aria-label={t('attendance.checkin.dismiss')}
              className="p-1 -m-1 rounded-lg text-red-200 hover:text-white hover:bg-white/10"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Action */}
        <div className="mt-5">
          {state === 'checked-in' ? (
            <button
              onClick={() => (confirmingOut ? checkOutMut.mutate() : setConfirmingOut(true))}
              disabled={submitting}
              className={`w-full min-h-14 rounded-2xl font-semibold flex items-center justify-center gap-2.5 transition-all active:scale-[0.98] disabled:opacity-60 ${
                confirmingOut ? 'bg-amber-300 text-amber-950' : 'bg-white/10 text-white border border-white/25 hover:bg-white/20'
              }`}
            >
              {checkOutMut.isPending
                ? <><Spinner /> {t('attendance.checkin.checkingOut')}</>
                : confirmingOut
                  ? <><LogOut className="w-5 h-5" /> {t('attendance.checkin.tapToConfirm')}</>
                  : <><LogOut className="w-5 h-5" /> {t('attendance.checkin.checkOut')}</>}
            </button>
          ) : state === 'checked-out' ? (
            <div className="w-full min-h-14 rounded-2xl bg-white/10 border border-white/20 flex items-center justify-center gap-2 text-sm font-medium text-white/85">
              <CheckCircle2 className="w-5 h-5 text-emerald-300" /> {t('attendance.checkin.allDone')}
            </div>
          ) : (
            <button
              onClick={handleCheckIn}
              disabled={submitting || todayLoading || !method}
              className="w-full min-h-14 rounded-2xl bg-white text-teal-900 text-base font-bold flex items-center justify-center gap-2.5 shadow-lg shadow-black/20 transition-all active:scale-[0.98] disabled:opacity-60 disabled:active:scale-100"
            >
              {faceIdMut.isPending
                ? <><Spinner dark /> {t('attendance.checkin.checkingIn')}</>
                : !method || todayLoading
                  ? <><Spinner dark /> {t('common.loading')}</>
                  : method.method === 'face_id'
                    ? <><Fingerprint className="w-5 h-5" /> {t('attendance.checkin.checkInFaceId')}</>
                    : <><ScanLine className="w-5 h-5" /> {t('attendance.checkin.checkIn')}</>}
            </button>
          )}
        </div>

        {(needsLocation || showWifiHint) && state !== 'checked-out' && (
          <div className="mt-3 flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-[11px] text-white/60">
            {needsLocation && (
              <span className="flex items-center gap-1.5">
                <MapPin className="w-3 h-3 shrink-0" />
                {method?.geofence_mode === 'enforce'
                  ? t('attendance.checkin.locationEnforced')
                  : t('attendance.checkin.locationRecorded')}
              </span>
            )}
            {showWifiHint && (
              <span className="flex items-center gap-1.5">
                <Wifi className="w-3 h-3 shrink-0" />
                {t('attendance.checkin.networkHint')}
              </span>
            )}
          </div>
        )}
      </Shell>

      {/* Success / Info feedback pill */}
      {feedback && feedback.tone !== 'error' && (
        <div
          role="status"
          className="animate-slide-up-sm fixed inset-x-4 z-[70] mx-auto max-w-md bottom-[calc(4.75rem+env(safe-area-inset-bottom))] md:bottom-6"
        >
          <div className={`flex items-start gap-2.5 rounded-2xl px-4 py-3 text-sm text-white shadow-xl ${
            feedback.tone === 'success' ? 'bg-emerald-600' : 'bg-sky-600'
          }`}>
            <CheckCircle2 className="w-4 h-4 mt-0.5 shrink-0" />
            <span className="flex-1">{feedback.text}</span>
            {feedback.href && (
              <Link to={feedback.href} className="font-semibold underline underline-offset-2 shrink-0">
                {feedback.hrefLabel}
              </Link>
            )}
          </div>
        </div>
      )}

      {scanning && (
        <QrScanSheet
          busy={qrMut.isPending}
          errorText={scanErrorText}
          onRetryScan={() => setScanErrorText('')}
          onClose={() => { setScanning(false); setScanErrorText(''); warmCoordsRef.current = null; }}
          onCameraReady={() => {
            if (needsLocation) {
              warmCoordsRef.current = getGeolocationResult(true);
            }
          }}
          onToken={(token) => { setScanErrorText(''); qrMut.mutate(token); }}
        />
      )}
    </>
  );
}

/** Isolated 1-second clock/elapsed ticker so CheckInCard buttons & scanner sheet stay calm. */
function CheckInClockDisplay({
  state,
  record,
  tz,
  method,
}: {
  state: TodayState;
  record: AttendanceRecord | null;
  tz: string;
  method: AttendanceMethodResponse | undefined;
}) {
  const { t } = useTranslation();
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  const dateLine = formatZonedDateLine(now, tz);
  const clock = formatZonedClock(now, tz);
  const { hour, minute } = zonedParts(now, tz);
  const standing = shiftStanding(
    hour * 60 + minute,
    parseScheduleTime(method?.shift_start_time),
    method?.grace_minutes ?? 0,
  );
  const startLabel = formatScheduleTime(method?.shift_start_time);

  const elapsed =
    record?.check_in_at && state === 'checked-in'
      ? formatDuration(now.getTime() - new Date(record.check_in_at).getTime())
      : null;
  const overnight = record?.check_in_at && state === 'checked-in'
    ? isPriorLocalDay(record.check_in_at, tz, now)
    : false;

  const methodLabel =
    method?.method === 'face_id' ? t('attendance.checkin.faceId') : t('attendance.checkin.qrCode');
  const MethodIcon = method?.method === 'face_id' ? Fingerprint : QrCode;

  return (
    <>
      {/* Date + method */}
      <div className="flex items-start justify-between gap-3">
        <p className="text-sm text-white/75">{dateLine}</p>
        {method && (
          <span className="inline-flex items-center gap-1.5 rounded-full bg-white/10 px-2.5 py-1 text-[11px] font-medium text-white/85 shrink-0">
            <MethodIcon className="w-3 h-3" /> {methodLabel}
          </span>
        )}
      </div>

      {/* Clock / elapsed */}
      <div className="mt-3">
        {state === 'checked-in' && elapsed ? (
          <>
            <p className="text-4xl sm:text-5xl font-bold tabular-nums leading-none">{elapsed}</p>
            <p className="text-sm text-white/75 mt-2">
              {overnight
                ? t('attendance.checkin.onTheClockSinceDate', {
                    dateTime: formatZonedDateTime(record?.check_in_at, tz),
                  })
                : t('attendance.checkin.onTheClockSince', {
                    time: formatZonedTime(record?.check_in_at, tz),
                  })}
              {record?.status === 'late' && (
                <span className="text-amber-300"> · {t('attendance.checkin.markedLate')}</span>
              )}
            </p>
          </>
        ) : (
          <>
            <p
              className="text-4xl sm:text-5xl font-bold tabular-nums leading-none"
              aria-label={t('attendance.checkin.currentTime', { time: clock })}
            >
              {clock}
            </p>
            <p className="text-sm text-white/75 mt-2">
              {state === 'checked-out' ? (
                <>
                  {formatZonedTime(record?.check_in_at, tz)} –{' '}
                  {formatZonedTime(record?.check_out_at, tz)}
                  {record?.hours_worked && (
                    <> · {t('attendance.checkin.hoursShort', { count: record.hours_worked })}</>
                  )}
                </>
              ) : startLabel && standing.kind !== 'unknown' ? (
                <>
                  {t('attendance.checkin.shiftStarts', { time: startLabel })}
                  {standing.kind === 'late' ? (
                    <span className="text-amber-300">
                      {' '}
                      ·{' '}
                      {t('attendance.checkin.pastGrace', {
                        duration: formatDuration(standing.minutes * 60_000),
                      })}
                    </span>
                  ) : (
                    <span className="text-emerald-300"> · {t('attendance.checkin.onTime')}</span>
                  )}
                </>
              ) : (
                t('attendance.checkin.ready')
              )}
            </p>
          </>
        )}
      </div>
    </>
  );
}

/** Deep teal so white text clears WCAG AA, unlike the raw accent gradient. */
function Shell({ children }: { children: React.ReactNode }) {
  return (
    <section className="relative overflow-hidden rounded-3xl bg-gradient-to-br from-teal-900 via-teal-800 to-emerald-800 text-white p-5 sm:p-6 shadow-[0_18px_40px_-16px_var(--glow)]">
      <div aria-hidden className="pointer-events-none absolute -top-16 -right-12 h-48 w-48 rounded-full bg-emerald-400/20 blur-3xl" />
      <div className="relative">{children}</div>
    </section>
  );
}

function Spinner({ dark }: { dark?: boolean }) {
  return (
    <span
      aria-hidden
      className={`inline-block h-4 w-4 animate-spin rounded-full border-2 border-transparent ${
        dark ? 'border-t-teal-900 border-r-teal-900/40' : 'border-t-white border-r-white/40'
      }`}
    />
  );
}
