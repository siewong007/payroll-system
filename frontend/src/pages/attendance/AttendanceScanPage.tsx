import { useEffect, useRef, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import {
  CheckCircle2, XCircle, Loader2, MapPin, QrCode, Info, RefreshCw, Navigation, WifiOff, Clock,
} from 'lucide-react';
import { checkInQr, getAttendanceMethod } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { classifyCheckInError, getGeolocationResult } from '@/lib/attendance';
import { getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatTimeSeconds } from '@/lib/format';
import { hasOnlyEmployeeRole } from '@/lib/roles';

type CheckInApiError = { response?: { status?: number; data?: { error?: string } } };

type State =
  | 'authenticating'
  | 'locating'
  | 'checking-in'
  | 'success'
  | 'already-checked-in'
  | 'location-required'
  | 'outside-geofence'
  | 'offsite-network'
  | 'expired-code'
  | 'stale-session'
  | 'no-employee'
  | 'error'
  | 'no-token';

export function AttendanceScanPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading, user } = useAuth();
  const token = searchParams.get('token');

  const [state, setState] = useState<State>(token ? 'authenticating' : 'no-token');
  const [error, setError] = useState('');
  const [record, setRecord] = useState<{ check_in_at: string } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const tRef = useRef(t);
  useEffect(() => {
    tRef.current = t;
  }, [t]);
  const inflightKeyRef = useRef<string | null>(null);

  const portalOnly = hasOnlyEmployeeRole(user);
  const attendanceHref = portalOnly ? '/portal/attendance' : '/my/attendance';
  const fallbackHref = portalOnly ? '/portal/attendance' : '/';

  const userId = user?.id;
  const userEmployeeId = user?.employee_id;
  const mustChangePassword = user?.must_change_password;

  useEffect(() => {
    if (!token) {
      setState('no-token');
      return;
    }
    if (authLoading) {
      setState('authenticating');
      return;
    }

    if (!isAuthenticated) {
      // Redirect to login, come back here after
      navigate(`/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`, { replace: true });
      return;
    }

    if (mustChangePassword) {
      navigate('/change-password', { replace: true });
      return;
    }

    if (!userEmployeeId) {
      setState('no-employee');
      setError(tRef.current('attendance.scan.noEmployeeProfile'));
      return;
    }

    let cancelled = false;
    const checkInKey = `${token}:${userId ?? ''}:${attempt}`;
    if (inflightKeyRef.current === checkInKey) return;
    inflightKeyRef.current = checkInKey;

    const executeCheckIn = async () => {
      try {
        // Ask the server whether coordinates matter before paying for a fix.
        let needsLocation: boolean;
        try {
          const method = await getAttendanceMethod();
          needsLocation = method.geofence_mode !== 'none';
        } catch {
          // Bootstrap unavailable — try for a fix, the server decides anyway.
          needsLocation = true;
        }

        if (cancelled) return;
        setState(needsLocation ? 'locating' : 'checking-in');
        const geo = await getGeolocationResult(needsLocation);
        if (cancelled) return;

        setState('checking-in');
        try {
          const result = await checkInQr(token, geo.coords?.latitude, geo.coords?.longitude);
          if (cancelled) return;
          setRecord(result);
          setState('success');
          void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
          void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
        } catch (e: unknown) {
          if (cancelled) return;
          const err = e as CheckInApiError;
          const raw = err.response?.data?.error;
          const message = getErrorMessage(e, tRef.current('attendance.scan.checkInFailed'));
          setError(message);

          switch (classifyCheckInError(raw ?? message)) {
            case 'already-checked-in':
              setState('already-checked-in');
              void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
              void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
              break;
            case 'outside-geofence':
              setState('outside-geofence');
              break;
            case 'offsite-network':
              setState('offsite-network');
              break;
            case 'expired-code':
              setState('expired-code');
              break;
            case 'stale-session':
              setState('stale-session');
              break;
            case 'location-permission':
              if (geo.errorCode === 2 || geo.errorCode === 3) {
                setError(tRef.current('attendance.checkin.locationTimeout'));
              }
              setState('location-required');
              break;
            default:
              setState('error');
          }
        }
      } finally {
        if (inflightKeyRef.current === checkInKey) {
          inflightKeyRef.current = null;
        }
      }
    };

    void executeCheckIn();
    return () => {
      cancelled = true;
      if (inflightKeyRef.current === checkInKey) {
        inflightKeyRef.current = null;
      }
    };
  }, [token, authLoading, isAuthenticated, navigate, userId, userEmployeeId, mustChangePassword, queryClient, attempt]);

  const formatTime = (iso: string) => formatTimeSeconds(iso);

  const retry = () => setAttempt((n) => n + 1);

  return (
    <main className="theme-portal min-h-[100dvh] bg-[var(--background)] text-[var(--foreground)] flex items-center justify-center p-6 pt-[max(1.5rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <div className="w-full max-w-sm">

        {/* Logo */}
        <BrandLogo variant="lockup-light" className="h-10 w-auto mx-auto mb-8 dark:hidden" />
        <BrandLogo variant="lockup-dark" className="hidden dark:block h-10 w-auto mx-auto mb-8 rounded-xl" />

        {/* Card */}
        <div className="card !rounded-3xl !p-6 sm:!p-8 shadow-lg flex flex-col items-center text-center gap-5">

          {/* Authenticating */}
          {state === 'authenticating' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-20 h-20 bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center">
                <Loader2 className="w-9 h-9 text-teal-600 dark:text-teal-400 animate-spin" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('common.loading')}</h1>
              </div>
            </div>
          )}

          {/* Locating */}
          {state === 'locating' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-20 h-20 bg-sky-50 dark:bg-sky-500/15 rounded-full flex items-center justify-center">
                <MapPin className="w-9 h-9 text-sky-500 dark:text-sky-400 motion-safe:animate-pulse" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.locating')}</h1>
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.locatingBody')}</p>
              </div>
            </div>
          )}

          {/* Checking in */}
          {state === 'checking-in' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-20 h-20 bg-violet-50 dark:bg-violet-500/15 rounded-full flex items-center justify-center">
                <Loader2 className="w-9 h-9 text-violet-500 dark:text-violet-400 animate-spin" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.checkingIn')}</h1>
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.checkingInBody')}</p>
              </div>
            </div>
          )}

          {/* Success */}
          {state === 'success' && (
            <>
              <div role="status" className="flex flex-col items-center gap-4 w-full">
                <div className="w-20 h-20 bg-emerald-50 dark:bg-emerald-500/15 rounded-full flex items-center justify-center">
                  <CheckCircle2 className="w-10 h-10 text-emerald-500 dark:text-emerald-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.checkedIn')}</h1>
                  {record && (
                    <p className="text-sm text-gray-500 dark:text-slate-400 tabular-nums">
                      {t('attendance.scan.welcome', { name: user?.full_name })}
                      <br />
                      {t('attendance.scan.atTime', { time: formatTime(record.check_in_at) })}
                    </p>
                  )}
                </div>
                <div className="w-full bg-emerald-50 dark:bg-emerald-500/15 rounded-2xl p-4 text-emerald-700 dark:text-emerald-200 text-sm">
                  {t('attendance.scan.recorded')}
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Already checked in — not a failure: attendance is recorded. */}
          {state === 'already-checked-in' && (
            <>
              <div role="status" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-sky-50 dark:bg-sky-500/15 rounded-full flex items-center justify-center">
                  <Info className="w-10 h-10 text-sky-500 dark:text-sky-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.alreadyTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">
                    {t('attendance.scan.alreadyBody')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Location required — actionable, with a retry on the same token. */}
          {state === 'location-required' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                  <MapPin className="w-10 h-10 text-amber-500 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.locationNeeded')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('attendance.scan.enableLocationRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-site geofence */}
          {state === 'outside-geofence' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                  <Navigation className="w-10 h-10 text-amber-500 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.notAtLocation')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('attendance.scan.movedRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-network */}
          {state === 'offsite-network' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                  <WifiOff className="w-10 h-10 text-amber-500 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.offsiteTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">
                    {t('attendance.checkin.offsiteSuffix', { message: translateServerMessage(error) })}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Expired or revoked token — open scanner to read a fresh kiosk code */}
          {state === 'expired-code' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                  <Clock className="w-10 h-10 text-amber-500 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.expiredTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.openScanner')}
              </button>
            </>
          )}

          {/* Unclosed previous session */}
          {state === 'stale-session' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                  <Clock className="w-10 h-10 text-amber-500 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Account without employee profile */}
          {state === 'no-employee' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center">
                  <XCircle className="w-10 h-10 text-red-500 dark:text-red-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{error}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-2xl"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}

          {/* Error */}
          {state === 'error' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center">
                  <XCircle className="w-10 h-10 text-red-500 dark:text-red-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-2xl"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* No token */}
          {state === 'no-token' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-20 h-20 bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center">
                  <QrCode className="w-9 h-9 text-gray-500 dark:text-slate-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.invalidQr')}</h1>
                  <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.invalidQrBody')}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-2xl"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}
        </div>
      </div>
    </main>
  );
}
