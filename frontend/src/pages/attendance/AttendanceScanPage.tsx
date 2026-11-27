import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import {
  Camera, CheckCircle2, Clock, Fingerprint, Info, Loader2, MapPin, Navigation, QrCode,
  RefreshCw, ShieldAlert, Smartphone, WifiOff, XCircle,
} from 'lucide-react';
import {
  beginFaceIdCheckIn, checkInQr, checkInQrFaceId, enrollAttendanceDevice,
  getAttendanceMethod, getMyAttendanceDevices, issueDeviceChallenge,
  type AttendanceCheckOptions, type AttendanceMethodResponse,
} from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LivePhotoCaptureModal } from '@/components/attendance/LivePhotoCaptureModal';
import { classifyCheckInError, getGeolocationResult } from '@/lib/attendance';
import {
  detectDefaultDeviceLabel, deviceScopeKey, getEnrolledDeviceId, getOrCreateDeviceKey,
  isDeviceKeySupported, setEnrolledDeviceId, signDeviceChallenge,
} from '@/lib/deviceKey';
import { getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatTimeSeconds } from '@/lib/format';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { getPasskeyCredential, type AuthenticationResponseJSON } from '@/lib/webauthn';

type CheckInApiError = { response?: { status?: number; data?: { error?: string } } };

type State =
  | 'authenticating'
  | 'locating'
  | 'needs-face-id'
  | 'needs-photo'
  | 'checking-in'
  | 'success'
  | 'already-checked-in'
  | 'location-required'
  | 'outside-geofence'
  | 'offsite-network'
  | 'device-not-approved'
  | 'photo-required'
  | 'expired-code'
  | 'stale-session'
  | 'no-employee'
  | 'error'
  | 'no-token';

const PENDING_QR_TOKEN_KEY = 'pending_attendance_qr_token';

function extractTokenParam(raw: string): string | null {
  const trimmed = raw.replace(/^[?#]/, '');
  if (!trimmed) return null;
  const val = new URLSearchParams(trimmed).get('token')?.trim();
  return val || null;
}

function readInitialScanToken(locationHash: string, locationSearch: string): string | null {
  const hashToken =
    extractTokenParam(typeof window !== 'undefined' ? window.location.hash : '') ??
    extractTokenParam(locationHash);
  if (hashToken) return hashToken;

  const queryToken =
    extractTokenParam(typeof window !== 'undefined' ? window.location.search : '') ??
    extractTokenParam(locationSearch);
  if (queryToken) return queryToken;

  try {
    const stored = sessionStorage.getItem(PENDING_QR_TOKEN_KEY)?.trim();
    return stored || null;
  } catch {
    return null;
  }
}

export function AttendanceScanPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const location = useLocation();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading, user } = useAuth();
  const [token] = useState<string | null>(() =>
    readInitialScanToken(location.hash, location.search),
  );

  const [state, setState] = useState<State>(token ? 'authenticating' : 'no-token');
  const [error, setError] = useState('');
  const [record, setRecord] = useState<{ check_in_at: string } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [methodMeta, setMethodMeta] = useState<AttendanceMethodResponse | null>(null);
  const [pendingCred, setPendingCred] = useState<{
    challengeId: string;
    credential: AuthenticationResponseJSON;
  } | null>(null);
  const [geoCoords, setGeoCoords] = useState<{ latitude?: number; longitude?: number; errorCode: number | null }>({
    errorCode: null,
  });
  const [photoModalOpen, setPhotoModalOpen] = useState(false);
  const [submittingAction, setSubmittingAction] = useState(false);
  const [enrollingDevice, setEnrollingDevice] = useState(false);
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
  const companyId = user?.company_id;
  const mustChangePassword = user?.must_change_password;
  const scopeKey = deviceScopeKey(companyId, userEmployeeId);

  const buildScanOptions = useCallback(
    async (
      method: AttendanceMethodResponse | null,
      photoBase64?: string | null,
    ): Promise<AttendanceCheckOptions | undefined> => {
      let deviceProof;
      if (method && (method.device_mode ?? 'none') !== 'none' && isDeviceKeySupported()) {
        let deviceId = await getEnrolledDeviceId(scopeKey);
        if (!deviceId && method.device_auto_approve_first) {
          try {
            const existing = await getMyAttendanceDevices();
            if (existing.filter((d) => d.status !== 'revoked').length === 0) {
              const { publicKeyRawB64Url } = await getOrCreateDeviceKey();
              const enrolled = await enrollAttendanceDevice({
                public_key_raw: publicKeyRawB64Url,
                device_label: detectDefaultDeviceLabel(),
              });
              await setEnrolledDeviceId(enrolled.id, scopeKey);
              deviceId = enrolled.id;
            }
          } catch {
            // Proceed; server checks policy
          }
        }
        if (deviceId) {
          try {
            const challenge = await issueDeviceChallenge('check_in');
            const proof = await signDeviceChallenge(challenge, 'check_in', scopeKey, deviceId);
            if (proof) deviceProof = proof;
          } catch {
            // Server handles missing proof according to device_mode
          }
        }
      }
      if (!deviceProof && !photoBase64) return undefined;
      return {
        ...(deviceProof ? { device_proof: deviceProof } : {}),
        ...(photoBase64 ? { photo_base64: photoBase64 } : {}),
      };
    },
    [scopeKey],
  );

  const applyCheckInError = useCallback(
    (e: unknown, errorCode: number | null) => {
      const err = e as CheckInApiError;
      const raw = err.response?.data?.error;
      const message = getErrorMessage(e, tRef.current('attendance.scan.checkInFailed'));
      setError(message);

      switch (classifyCheckInError(raw ?? message)) {
        case 'already-checked-in':
          try {
            sessionStorage.removeItem(PENDING_QR_TOKEN_KEY);
          } catch {
            // Ignore
          }
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
        case 'device-not-approved':
          setState('device-not-approved');
          break;
        case 'photo-required':
          setState('photo-required');
          break;
        case 'expired-code':
          try {
            sessionStorage.removeItem(PENDING_QR_TOKEN_KEY);
          } catch {
            // Ignore
          }
          setState('expired-code');
          break;
        case 'stale-session':
          setState('stale-session');
          break;
        case 'location-permission':
          if (errorCode === 2 || errorCode === 3) {
            setError(tRef.current('attendance.checkin.locationTimeout'));
          }
          setState('location-required');
          break;
        default:
          setState('error');
      }
    },
    [queryClient],
  );

  useEffect(() => {
    if (!token) {
      setState('no-token');
      return;
    }
    if (
      typeof window !== 'undefined' &&
      (extractTokenParam(window.location.hash) || extractTokenParam(window.location.search))
    ) {
      window.history.replaceState(null, '', window.location.pathname);
    }
    if (authLoading) {
      setState('authenticating');
      return;
    }

    if (!isAuthenticated) {
      try {
        sessionStorage.setItem(PENDING_QR_TOKEN_KEY, token);
      } catch {
        // Ignore storage restrictions
      }
      navigate(`/login?redirect=${encodeURIComponent('/attendance/scan')}`, { replace: true });
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
        let method: AttendanceMethodResponse | null = null;
        let needsLocation = true;
        try {
          method = await getAttendanceMethod();
          if (!cancelled) setMethodMeta(method);
          needsLocation = method.geofence_mode !== 'none';
        } catch {
          needsLocation = true;
        }

        if (cancelled) return;
        setState(needsLocation ? 'locating' : 'checking-in');
        const geo = await getGeolocationResult(needsLocation);
        if (cancelled) return;
        setGeoCoords({
          latitude: geo.coords?.latitude,
          longitude: geo.coords?.longitude,
          errorCode: geo.errorCode,
        });

        if (method?.method === 'qr_and_face_id') {
          setState('needs-face-id');
          return;
        }
        if ((method?.photo_mode ?? 'none') !== 'none') {
          setState('needs-photo');
          setPhotoModalOpen(true);
          return;
        }

        setState('checking-in');
        try {
          const opts = await buildScanOptions(method, null);
          const result = opts
            ? await checkInQr(token, geo.coords?.latitude, geo.coords?.longitude, opts)
            : await checkInQr(token, geo.coords?.latitude, geo.coords?.longitude);
          if (cancelled) return;
          try {
            sessionStorage.removeItem(PENDING_QR_TOKEN_KEY);
          } catch {
            // Ignore storage restrictions
          }
          setRecord(result);
          setState('success');
          void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
          void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
        } catch (e: unknown) {
          if (cancelled) return;
          applyCheckInError(e, geo.errorCode);
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
  }, [
    token,
    authLoading,
    isAuthenticated,
    navigate,
    userId,
    userEmployeeId,
    mustChangePassword,
    queryClient,
    attempt,
    applyCheckInError,
    buildScanOptions,
  ]);

  const submitScannedCheckIn = async (
    photoBase64: string | null,
    faceIdData?: { challengeId: string; credential: AuthenticationResponseJSON } | null,
  ) => {
    if (!token) return;
    setSubmittingAction(true);
    setState('checking-in');
    try {
      const opts = await buildScanOptions(methodMeta, photoBase64);
      const result = faceIdData
        ? await checkInQrFaceId(
            token,
            faceIdData.challengeId,
            faceIdData.credential,
            geoCoords.latitude,
            geoCoords.longitude,
            opts,
          )
        : opts
          ? await checkInQr(token, geoCoords.latitude, geoCoords.longitude, opts)
          : await checkInQr(token, geoCoords.latitude, geoCoords.longitude);
      setPhotoModalOpen(false);
      setPendingCred(null);
      try {
        sessionStorage.removeItem(PENDING_QR_TOKEN_KEY);
      } catch {
        // Ignore
      }
      setRecord(result);
      setState('success');
      void queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
    } catch (e) {
      setPhotoModalOpen(false);
      applyCheckInError(e, geoCoords.errorCode);
    } finally {
      setSubmittingAction(false);
    }
  };

  const handleFaceIdConfirm = async () => {
    if (!token) return;
    setSubmittingAction(true);
    try {
      const { challenge_id, options } = await beginFaceIdCheckIn();
      const credential = await getPasskeyCredential(options.publicKey);
      if ((methodMeta?.photo_mode ?? 'none') !== 'none') {
        setPendingCred({ challengeId: challenge_id, credential });
        setState('needs-photo');
        setPhotoModalOpen(true);
        setSubmittingAction(false);
        return;
      }
      await submitScannedCheckIn(null, { challengeId: challenge_id, credential });
    } catch (e) {
      setSubmittingAction(false);
      applyCheckInError(e, geoCoords.errorCode);
    }
  };

  const handleEnrollThisDevice = async () => {
    setEnrollingDevice(true);
    try {
      const { publicKeyRawB64Url } = await getOrCreateDeviceKey();
      const enrolled = await enrollAttendanceDevice({
        public_key_raw: publicKeyRawB64Url,
        device_label: detectDefaultDeviceLabel(),
      });
      await setEnrolledDeviceId(enrolled.id, scopeKey);
      if (enrolled.status === 'approved') {
        setAttempt((n) => n + 1);
      } else {
        setError(t('attendance.device.enrolledPending'));
      }
    } catch (e) {
      setError(getErrorMessage(e, t('attendance.device.enrollFailed')));
    } finally {
      setEnrollingDevice(false);
    }
  };

  const formatTime = (iso: string) => formatTimeSeconds(iso);

  const retry = () => setAttempt((n) => n + 1);

  return (
    <main className="theme-portal min-h-[100dvh] bg-[var(--background)] text-[var(--foreground)] flex items-center justify-center p-6 pt-[max(1.5rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <div className="w-full max-w-sm">

        {/* Logo */}
        <BrandLogo variant="lockup-auto" className="h-10 w-auto mx-auto mb-6 rounded-lg" />

        {/* Card */}
        <div className="card !p-6 sm:!p-7 flex flex-col items-center text-center gap-5">

          {/* Authenticating */}
          {state === 'authenticating' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-16 h-16 bg-[var(--surface-subtle)] border border-[var(--border)] rounded-xl flex items-center justify-center">
                <Loader2 className="w-8 h-8 text-[var(--accent-1)] animate-spin" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('common.loading')}</h1>
              </div>
            </div>
          )}

          {/* Needs explicit Face ID confirmation (qr_and_face_id) */}
          {state === 'needs-face-id' && (
            <>
              <div className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-[var(--accent-soft)] rounded-xl flex items-center justify-center">
                  <Fingerprint className="w-8 h-8 text-[var(--accent-1)]" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">
                    {t('attendance.checkin.qrFaceIdStepTitle')}
                  </h1>
                  <p className="text-sm text-[var(--muted-foreground)]">
                    {t('attendance.checkin.qrFaceIdStepBody')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                disabled={submittingAction}
                onClick={() => void handleFaceIdConfirm()}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <Fingerprint className="w-4 h-4" aria-hidden="true" />
                {t('attendance.checkin.confirmFaceIdButton')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Needs live camera photo */}
          {(state === 'needs-photo' || state === 'photo-required') && (
            <>
              <div className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-[var(--accent-soft)] rounded-xl flex items-center justify-center">
                  <Camera className="w-8 h-8 text-[var(--accent-1)]" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">
                    {t('attendance.photo.checkInTitle')}
                  </h1>
                  <p className="text-sm text-[var(--muted-foreground)]">
                    {error ? translateServerMessage(error) : t('attendance.photo.requiredDescription')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setPhotoModalOpen(true)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <Camera className="w-4 h-4" aria-hidden="true" />
                {t('attendance.photo.openCamera')}
              </button>
            </>
          )}

          {/* Bound device not enrolled / pending / revoked */}
          {state === 'device-not-approved' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <ShieldAlert className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">
                    {t('attendance.device.blockedTitle')}
                  </h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              {isDeviceKeySupported() && (
                <button
                  type="button"
                  disabled={enrollingDevice}
                  onClick={() => void handleEnrollThisDevice()}
                  className="btn-primary w-full min-h-11 justify-center rounded-lg"
                >
                  <Smartphone className="w-4 h-4" aria-hidden="true" />
                  {enrollingDevice ? t('common.loading') : t('attendance.device.enrollButton')}
                </button>
              )}
              <button
                type="button"
                onClick={retry}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('common.retry')}
              </button>
            </>
          )}

          {/* Locating */}
          {state === 'locating' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-16 h-16 bg-[var(--accent-soft)] rounded-xl flex items-center justify-center">
                <MapPin className="w-8 h-8 text-[var(--accent-1)] motion-safe:animate-pulse" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.locating')}</h1>
                <p className="text-sm text-[var(--muted-foreground)]">{t('attendance.scan.locatingBody')}</p>
              </div>
            </div>
          )}

          {/* Checking in */}
          {state === 'checking-in' && (
            <div role="status" className="flex flex-col items-center gap-4">
              <div className="w-16 h-16 bg-[var(--accent-soft)] rounded-xl flex items-center justify-center">
                <Loader2 className="w-8 h-8 text-[var(--accent-1)] animate-spin" aria-hidden="true" />
              </div>
              <div>
                <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.checkingIn')}</h1>
                <p className="text-sm text-[var(--muted-foreground)]">{t('attendance.scan.checkingInBody')}</p>
              </div>
            </div>
          )}

          {/* Success */}
          {state === 'success' && (
            <>
              <div role="status" className="flex flex-col items-center gap-4 w-full">
                <div className="w-16 h-16 bg-emerald-50 dark:bg-emerald-500/15 border border-emerald-200 dark:border-emerald-500/30 rounded-xl flex items-center justify-center">
                  <CheckCircle2 className="w-8 h-8 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.checkedIn')}</h1>
                  {record && (
                    <p className="text-sm text-[var(--muted-foreground)] tabular-nums">
                      {t('attendance.scan.welcome', { name: user?.full_name })}
                      <br />
                      {t('attendance.scan.atTime', { time: formatTime(record.check_in_at) })}
                    </p>
                  )}
                </div>
                <div className="w-full bg-emerald-50 dark:bg-emerald-500/15 border border-emerald-200 dark:border-emerald-500/30 rounded-lg p-3.5 text-emerald-700 dark:text-emerald-200 text-sm">
                  {t('attendance.scan.recorded')}
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Already checked in — not a failure: attendance is recorded. */}
          {state === 'already-checked-in' && (
            <>
              <div role="status" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-[var(--accent-soft)] rounded-xl flex items-center justify-center">
                  <Info className="w-8 h-8 text-[var(--accent-1)]" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.alreadyTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">
                    {t('attendance.scan.alreadyBody')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Location required — actionable, with a retry on the same token. */}
          {state === 'location-required' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <MapPin className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.locationNeeded')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('attendance.scan.enableLocationRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-site geofence */}
          {state === 'outside-geofence' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <Navigation className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.notAtLocation')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('attendance.scan.movedRetry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-network */}
          {state === 'offsite-network' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <WifiOff className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.offsiteTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">
                    {t('attendance.checkin.offsiteSuffix', { message: translateServerMessage(error) })}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Expired or revoked token — open scanner to read a fresh kiosk code */}
          {state === 'expired-code' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <Clock className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.expiredTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.openScanner')}
              </button>
            </>
          )}

          {/* Unclosed previous session */}
          {state === 'stale-session' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-amber-50 dark:bg-amber-500/15 border border-amber-200 dark:border-amber-500/30 rounded-xl flex items-center justify-center">
                  <Clock className="w-8 h-8 text-amber-600 dark:text-amber-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.viewMyAttendance')}
              </button>
            </>
          )}

          {/* Account without employee profile */}
          {state === 'no-employee' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 border border-red-200 dark:border-red-500/30 rounded-xl flex items-center justify-center">
                  <XCircle className="w-8 h-8 text-red-600 dark:text-red-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{error}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}

          {/* Error */}
          {state === 'error' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 border border-red-200 dark:border-red-500/30 rounded-xl flex items-center justify-center">
                  <XCircle className="w-8 h-8 text-red-600 dark:text-red-400" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.failedTitle')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{translateServerMessage(error)}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={retry}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                <RefreshCw className="w-4 h-4" aria-hidden="true" /> {t('common.retry')}
              </button>
              <button
                type="button"
                onClick={() => navigate(attendanceHref)}
                className="btn-secondary w-full min-h-11 justify-center rounded-lg"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* No token */}
          {state === 'no-token' && (
            <>
              <div role="alert" className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 bg-[var(--surface-subtle)] border border-[var(--border)] rounded-xl flex items-center justify-center">
                  <QrCode className="w-8 h-8 text-[var(--muted-foreground)]" aria-hidden="true" />
                </div>
                <div>
                  <h1 className="text-xl font-bold text-[var(--foreground)] mb-1">{t('attendance.scan.invalidQr')}</h1>
                  <p className="text-sm text-[var(--muted-foreground)]">{t('attendance.scan.invalidQrBody')}</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => navigate(fallbackHref)}
                className="btn-primary w-full min-h-11 justify-center rounded-lg"
              >
                {portalOnly ? t('attendance.scan.goToPortal') : t('attendance.scan.backToHome')}
              </button>
            </>
          )}
        </div>
      </div>

      <LivePhotoCaptureModal
        open={photoModalOpen}
        title={t('attendance.photo.checkInTitle')}
        description={
          methodMeta?.photo_mode === 'required'
            ? t('attendance.photo.requiredDescription')
            : t('attendance.photo.optionalDescription')
        }
        required={methodMeta?.photo_mode === 'required'}
        submitting={submittingAction}
        onConfirm={(photoBase64) => void submitScannedCheckIn(photoBase64, pendingCred)}
        onCancel={() => {
          setPhotoModalOpen(false);
          if (state === 'needs-photo') {
            navigate(attendanceHref);
          }
        }}
      />
    </main>
  );
}
