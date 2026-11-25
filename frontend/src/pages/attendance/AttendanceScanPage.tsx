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
          const err = e as { response?: { status?: number; data?: { error?: string } } };
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
    <div className="theme-portal min-h-screen bg-[var(--background)] text-[var(--foreground)] flex items-center justify-center p-6">
      <div className="w-full max-w-sm">

        {/* Logo */}
        <BrandLogo variant="lockup-light" className="h-10 w-auto mx-auto mb-8 dark:hidden" />
        <BrandLogo variant="lockup-dark" className="hidden dark:block h-10 w-auto mx-auto mb-8 rounded-xl" />

        {/* Card */}
        <div className="bg-white dark:bg-slate-900 border border-gray-200/80 dark:border-slate-800 rounded-3xl shadow-lg p-8 flex flex-col items-center text-center gap-5">

          {/* Authenticating */}
          {state === 'authenticating' && (
            <>
              <div className="w-20 h-20 bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center">
                <Loader2 className="w-9 h-9 text-teal-600 dark:text-teal-400 animate-spin" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('common.loading')}</h2>
              </div>
            </>
          )}

          {/* Locating */}
          {state === 'locating' && (
            <>
              <div className="w-20 h-20 bg-sky-50 dark:bg-sky-500/15 rounded-full flex items-center justify-center">
                <MapPin className="w-9 h-9 text-sky-500 dark:text-sky-400 animate-pulse" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.locating')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.locatingBody')}</p>
              </div>
            </>
          )}

          {/* Checking in */}
          {state === 'checking-in' && (
            <>
              <div className="w-20 h-20 bg-violet-50 dark:bg-violet-500/15 rounded-full flex items-center justify-center">
                <Loader2 className="w-9 h-9 text-violet-500 dark:text-violet-400 animate-spin" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.checkingIn')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.checkingInBody')}</p>
              </div>
            </>
          )}

          {/* Success */}
          {state === 'success' && (
            <>
              <div className="w-20 h-20 bg-emerald-50 dark:bg-emerald-500/15 rounded-full flex items-center justify-center">
                <CheckCircle2 className="w-10 h-10 text-emerald-500 dark:text-emerald-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.checkedIn')}</h2>
                {record && (
                  <p className="text-sm text-gray-500 dark:text-slate-400">
                    {t('attendance.scan.welcome', { name: user?.full_name })}
                    <br />
                    {t('attendance.scan.atTime', { time: formatTime(record.check_in_at) })}
                  </p>
                )}
              </div>
              <div className="w-full bg-emerald-50 dark:bg-emerald-500/15 rounded-2xl p-4 text-emerald-700 dark:text-emerald-200 text-sm">
                {t('attendance.scan.recorded')}
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Already checked in — not a failure: attendance is recorded. */}
          {state === 'already-checked-in' && (
            <>
              <div className="w-20 h-20 bg-sky-50 dark:bg-sky-500/15 rounded-full flex items-center justify-center">
                <Info className="w-10 h-10 text-sky-500 dark:text-sky-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.alreadyTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">
                  {t('attendance.scan.alreadyBody')}
                </p>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Location required — actionable, with a retry on the same token. */}
          {state === 'location-required' && (
            <>
              <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                <MapPin className="w-10 h-10 text-amber-500 dark:text-amber-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.locationNeeded')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
              </div>
              <button
                type="button"
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('attendance.scan.enableLocationRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 rounded-2xl text-sm font-semibold hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-site geofence */}
          {state === 'outside-geofence' && (
            <>
              <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                <Navigation className="w-10 h-10 text-amber-500 dark:text-amber-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.notAtLocation')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
              </div>
              <button
                type="button"
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('attendance.scan.movedRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 rounded-2xl text-sm font-semibold hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-network */}
          {state === 'offsite-network' && (
            <>
              <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                <WifiOff className="w-10 h-10 text-amber-500 dark:text-amber-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.offsiteTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">
                  {t('attendance.checkin.offsiteSuffix', { message: translateServerMessage(error) })}
                </p>
              </div>
              <button
                type="button"
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 rounded-2xl text-sm font-semibold hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Expired or revoked token — open scanner to read a fresh kiosk code */}
          {state === 'expired-code' && (
            <>
              <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                <Clock className="w-10 h-10 text-amber-500 dark:text-amber-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.expiredTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.openScanner')}
              </button>
            </>
          )}

          {/* Unclosed previous session */}
          {state === 'stale-session' && (
            <>
              <div className="w-20 h-20 bg-amber-50 dark:bg-amber-500/15 rounded-full flex items-center justify-center">
                <Clock className="w-10 h-10 text-amber-500 dark:text-amber-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Account without employee profile */}
          {state === 'no-employee' && (
            <>
              <div className="w-20 h-20 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center">
                <XCircle className="w-10 h-10 text-red-500 dark:text-red-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{error}</p>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="w-full py-3 border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 rounded-2xl text-sm font-semibold hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}

          {/* Error */}
          {state === 'error' && (
            <>
              <div className="w-20 h-20 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center">
                <XCircle className="w-10 h-10 text-red-500 dark:text-red-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.failedTitle')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{translateServerMessage(error)}</p>
              </div>
              <button
                type="button"
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="w-full py-3 border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 rounded-2xl text-sm font-semibold hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* No token */}
          {state === 'no-token' && (
            <>
              <div className="w-20 h-20 bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center">
                <QrCode className="w-9 h-9 text-gray-400 dark:text-slate-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 dark:text-slate-100 mb-1">{t('attendance.scan.invalidQr')}</h2>
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('attendance.scan.invalidQrBody')}</p>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
