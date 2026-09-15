import { useCallback, useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AnimatePresence, motion } from 'framer-motion';
import {
  AlertCircle, CheckCircle2, Fingerprint, LogOut, MapPin, QrCode, RefreshCw, ScanLine, TriangleAlert,
} from 'lucide-react';
import {
  beginFaceIdCheckIn, checkInFaceId, checkInQr, checkOut, getAttendanceMethod, getMyTodayAttendance,
} from '@/api/attendance';
import { getDefaultSchedule } from '@/api/workSchedule';
import { getPasskeyCredential } from '@/lib/webauthn';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage } from '@/lib/utils';
import {
  buzz, classifyCheckInError, deriveTodayState, formatDuration, formatScheduleTime, formatZonedClock,
  formatZonedDateLine, formatZonedTime, getGeolocation, parseScheduleTime, shiftStanding, zonedParts,
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
  const queryClient = useQueryClient();

  const [now, setNow] = useState(() => new Date());
  const [scanning, setScanning] = useState(false);
  const [confirmingOut, setConfirmingOut] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [scanErrorText, setScanErrorText] = useState('');

  /**
   * A position resolved while the camera was already open. Fetching it here
   * rather than after the decode removes an up-to-8-second stall between
   * "code recognised" and "checked in".
   */
  const warmCoordsRef = useRef<Promise<GeolocationCoordinates | null> | null>(null);

  const { data: method } = useQuery({ queryKey: ['attendance-method'], queryFn: getAttendanceMethod });
  const { data: today, isError: todayFailed, isLoading: todayLoading, refetch: refetchToday } = useQuery({
    queryKey: ['attendance-today'],
    queryFn: getMyTodayAttendance,
    refetchInterval: 60_000,
  });
  // Advisory only — the card still works without it, so failures stay silent.
  const { data: scheduleData } = useQuery({
    queryKey: ['work-schedule-default'],
    queryFn: getDefaultSchedule,
    staleTime: 5 * 60_000,
    retry: false,
  });

  const tz = method?.timezone || FALLBACK_TZ;
  const needsLocation = (method?.geofence_mode ?? 'none') !== 'none';
  const record = today?.record ?? null;
  const state = deriveTodayState(record);

  // Live clock. A punch-clock screen that cannot tell you the time it is about
  // to record is missing its single most reassuring element.
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    if (!feedback) return;
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
        return { tone: 'error', text: message, href: '/portal/attendance', hrefLabel: t('attendance.checkin.viewHistory') };
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
        return { tone: 'error', text: t('attendance.checkin.locationRequired') };
      case 'no-passkey':
        return { tone: 'error', text: message, href: '/portal/profile', hrefLabel: t('attendance.checkin.addPasskey') };
      default:
        return { tone: 'error', text: message };
    }
  }, [refreshAttendance, t]);

  const faceIdMut = useMutation({
    mutationFn: async () => {
      // The ceremony must stay inside the tap's user activation, so the GPS
      // prompt waits until after it rather than racing the biometric sheet.
      const { challenge_id, options } = await beginFaceIdCheckIn();
      const credential = await getPasskeyCredential(options.publicKey);
      const coords = await getGeolocation(needsLocation);
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
      const coords = await (warmCoordsRef.current ?? getGeolocation(needsLocation));
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
      const coords = await getGeolocation(needsLocation);
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
      setFeedback({ tone: 'error', text: getErrorMessage(err, t('attendance.checkin.checkOutFailed')) });
    },
  });

  const submitting = faceIdMut.isPending || qrMut.isPending || checkOutMut.isPending;

  const handleCheckIn = () => {
    setFeedback(null);
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

  // ── Derived display ────────────────────────────────────────────────────────

  const dateLine = formatZonedDateLine(now, tz);
  const clock = formatZonedClock(now, tz);
  const { hour, minute } = zonedParts(now, tz);
  const schedule = scheduleData?.schedule ?? null;
  const standing = shiftStanding(hour * 60 + minute, parseScheduleTime(schedule?.start_time), schedule?.grace_minutes ?? 0);
  const startLabel = formatScheduleTime(schedule?.start_time);

  const elapsed = record?.check_in_at && state === 'checked-in'
    ? formatDuration(now.getTime() - new Date(record.check_in_at).getTime())
    : null;

  const methodLabel = method?.method === 'face_id' ? t('attendance.checkin.faceId') : t('attendance.checkin.qrCode');
  const MethodIcon = method?.method === 'face_id' ? Fingerprint : QrCode;

  // ── Blocking states ────────────────────────────────────────────────────────

  if (user && !user.employee_id) {
    return (
      <Shell>
        <div className="flex items-start gap-3">
          <AlertCircle className="w-5 h-5 text-amber-300 shrink-0 mt-0.5" />
          <div>
            <h2 className="font-semibold">{t('attendance.checkin.notSetUpTitle')}</h2>
            <p className="text-sm text-white/70 mt-1">
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
            <p className="text-sm text-white/70 mt-1">
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
        {/* Date + method */}
        <div className="flex items-start justify-between gap-3">
          <p className="text-sm text-white/70">{dateLine}</p>
          {method && (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-white/10 px-2.5 py-1 text-[11px] font-medium text-white/80 shrink-0">
              <MethodIcon className="w-3 h-3" /> {methodLabel}
            </span>
          )}
        </div>

        {/* Clock / elapsed */}
        <div className="mt-3">
          {state === 'checked-in' && elapsed ? (
            <>
              <p className="text-4xl sm:text-5xl font-bold tabular-nums leading-none">{elapsed}</p>
              <p className="text-sm text-white/70 mt-2">
                {t('attendance.checkin.onTheClockSince', { time: formatZonedTime(record?.check_in_at, tz) })}
                {record?.status === 'late' && <span className="text-amber-300"> · {t('attendance.checkin.markedLate')}</span>}
              </p>
            </>
          ) : (
            <>
              <p className="text-4xl sm:text-5xl font-bold tabular-nums leading-none" aria-label={t('attendance.checkin.currentTime', { time: clock })}>
                {clock}
              </p>
              <p className="text-sm text-white/70 mt-2">
                {state === 'checked-out' ? (
                  <>
                    {formatZonedTime(record?.check_in_at, tz)} – {formatZonedTime(record?.check_out_at, tz)}
                    {record?.hours_worked && <> · {t('attendance.checkin.hoursShort', { count: record.hours_worked })}</>}
                  </>
                ) : startLabel && standing.kind !== 'unknown' ? (
                  <>
                    {t('attendance.checkin.shiftStarts', { time: startLabel })}
                    {standing.kind === 'late'
                      ? <span className="text-amber-300"> · {t('attendance.checkin.pastGrace', { duration: formatDuration(standing.minutes * 60_000) })}</span>
                      : <span className="text-emerald-300"> · {t('attendance.checkin.onTime')}</span>}
                  </>
                ) : (
                  t('attendance.checkin.ready')
                )}
              </p>
            </>
          )}
        </div>

        {/* Absent notice */}
        {state === 'absent' && (
          <div className="mt-4 flex items-start gap-2 rounded-2xl bg-amber-400/15 border border-amber-300/30 px-3 py-2.5 text-sm text-amber-100">
            <TriangleAlert className="w-4 h-4 mt-0.5 shrink-0" />
            <span>{t('attendance.checkin.absentNotice')}</span>
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
            <div className="w-full min-h-14 rounded-2xl bg-white/10 border border-white/20 flex items-center justify-center gap-2 text-sm font-medium text-white/80">
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

        {needsLocation && state !== 'checked-out' && (
          <p className="mt-3 flex items-center justify-center gap-1.5 text-[11px] text-white/50">
            <MapPin className="w-3 h-3" />
            {method?.geofence_mode === 'enforce'
              ? t('attendance.checkin.locationEnforced')
              : t('attendance.checkin.locationRecorded')}
          </p>
        )}
      </Shell>

      {/* Feedback — anchored bottom, clear of the mobile tab bar and within
          thumb reach of the button that produced it. */}
      <AnimatePresence>
        {feedback && (
          <motion.div
            role="status"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 12 }}
            transition={{ duration: 0.18, ease: 'easeOut' }}
            className="fixed inset-x-4 z-[70] mx-auto max-w-md bottom-[calc(4.75rem+env(safe-area-inset-bottom))] md:bottom-6"
          >
            <div className={`flex items-start gap-2.5 rounded-2xl px-4 py-3 text-sm text-white shadow-xl ${
              feedback.tone === 'success' ? 'bg-emerald-600' : feedback.tone === 'info' ? 'bg-sky-600' : 'bg-red-600'
            }`}>
              {feedback.tone === 'error'
                ? <AlertCircle className="w-4 h-4 mt-0.5 shrink-0" />
                : <CheckCircle2 className="w-4 h-4 mt-0.5 shrink-0" />}
              <span className="flex-1">{feedback.text}</span>
              {feedback.href && (
                <Link to={feedback.href} className="font-semibold underline underline-offset-2 shrink-0">
                  {feedback.hrefLabel}
                </Link>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {scanning && (
        <QrScanSheet
          busy={qrMut.isPending}
          errorText={scanErrorText}
          onClose={() => { setScanning(false); setScanErrorText(''); }}
          onCameraReady={() => { warmCoordsRef.current = getGeolocation(needsLocation); }}
          onToken={(token) => { setScanErrorText(''); qrMut.mutate(token); }}
        />
      )}
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
