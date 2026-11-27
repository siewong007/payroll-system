import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle, Camera, CheckCircle2, Fingerprint, LogOut, MapPin, QrCode, RefreshCw, ScanLine,
  ShieldCheck, ShieldAlert, Smartphone, TriangleAlert, Wifi, X,
} from 'lucide-react';
import {
  beginFaceIdCheckIn, checkInFaceId, checkInQr, checkInQrFaceId, checkOut,
  enrollAttendanceDevice, getAttendanceMethod, getMyAttendanceDevices, getMyTodayAttendance,
  issueDeviceChallenge, type AttendanceMethodResponse, type AttendanceRecord,
  type AttendanceCheckOptions,
} from '@/api/attendance';
import { getPasskeyCredential, type AuthenticationResponseJSON } from '@/lib/webauthn';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage } from '@/lib/utils';
import {
  buzz, classifyCheckInError, deriveTodayState, formatDuration, formatScheduleTime, formatZonedClock,
  formatZonedDateLine, formatZonedDateTime, formatZonedTime, getGeolocationResult, isPriorLocalDay,
  parseScheduleTime, shiftStanding, zonedParts, type GeolocationResult, type TodayState,
} from '@/lib/attendance';
import {
  clearDeviceKey, detectDefaultDeviceLabel, deviceScopeKey, getEnrolledDeviceId,
  getOrCreateDeviceKey, isDeviceKeySupported, setEnrolledDeviceId, signDeviceChallenge,
} from '@/lib/deviceKey';
import { QrScanSheet } from './QrScanSheet';
import { LivePhotoCaptureModal } from './LivePhotoCaptureModal';
import { PersonalKioskPassModal } from './PersonalKioskPassModal';
import { Modal } from '@/components/ui/Modal';

const FALLBACK_TZ = 'Asia/Kuala_Lumpur';

type Feedback = { tone: 'success' | 'error' | 'info'; text: string; href?: string; hrefLabel?: string };
type PendingPhotoAction =
  | { kind: 'qr'; token: string }
  | { kind: 'face_id'; challengeId: string; credential: AuthenticationResponseJSON }
  | { kind: 'qr_face_id'; token: string; challengeId: string; credential: AuthenticationResponseJSON }
  | { kind: 'check_out' };

/**
 * The employee's whole attendance interaction, in one tap.
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
  const [pendingQrToken, setPendingQrToken] = useState<string | null>(null);
  const [pendingPhoto, setPendingPhoto] = useState<PendingPhotoAction | null>(null);
  const [kioskPassOpen, setKioskPassOpen] = useState(false);
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);

  const warmCoordsRef = useRef<Promise<GeolocationResult> | null>(null);
  const lastGeoErrorRef = useRef<number | null>(null);

  const hasEmployee = Boolean(user?.employee_id);
  const hasCompany = Boolean(user?.company_id);
  const scopeKey = deviceScopeKey(user?.company_id, user?.employee_id);

  const { data: method, isError: methodFailed, refetch: refetchMethod } = useQuery({
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
  const deviceMode = method?.device_mode ?? 'none';
  const photoMode = method?.photo_mode ?? 'none';
  const { data: myDevices = [], refetch: refetchMyDevices } = useQuery({
    queryKey: ['attendance-my-devices', scopeKey],
    queryFn: getMyAttendanceDevices,
    enabled: hasEmployee && hasCompany && deviceMode !== 'none',
  });

  useEffect(() => {
    let active = true;
    void getEnrolledDeviceId(scopeKey).then((id) => {
      if (active) setLocalDeviceId(id);
    });
    return () => {
      active = false;
    };
  }, [scopeKey]);

  const matchedDevice = useMemo(
    () => (localDeviceId ? myDevices.find((d) => d.id === localDeviceId) ?? null : null),
    [localDeviceId, myDevices],
  );

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
    void queryClient.invalidateQueries({ queryKey: ['attendance-my-devices'] });
  }, [queryClient]);

  const succeed = useCallback((text: string) => {
    buzz([18, 40, 18]);
    setFeedback({ tone: 'success', text });
    refreshAttendance();
  }, [refreshAttendance]);

  const explain = useCallback((err: unknown, fallback: string): Feedback => {
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
        return {
          tone: 'error',
          text: t('attendance.checkin.offsiteSuffix', { message }),
        };
      case 'location-permission':
        if (lastGeoErrorRef.current === 2 || lastGeoErrorRef.current === 3) {
          return { tone: 'error', text: t('attendance.checkin.locationTimeout') };
        }
        return { tone: 'error', text: t('attendance.checkin.locationRequired') };
      case 'device-not-approved':
        return { tone: 'error', text: message };
      case 'photo-required':
        return { tone: 'error', text: message };
      case 'no-passkey':
        return { tone: 'error', text: message, href: profileSecurityHref, hrefLabel: t('attendance.checkin.addPasskey') };
      default:
        return { tone: 'error', text: message };
    }
  }, [historyHref, profileSecurityHref, refreshAttendance, t]);

  const enrollMut = useMutation({
    mutationFn: async (replaceRevoked: boolean) => {
      if (replaceRevoked) {
        await clearDeviceKey();
      }
      const { publicKeyRawB64Url } = await getOrCreateDeviceKey();
      const device = await enrollAttendanceDevice({
        public_key_raw: publicKeyRawB64Url,
        device_label: detectDefaultDeviceLabel(),
      });
      await setEnrolledDeviceId(device.id, scopeKey);
      setLocalDeviceId(device.id);
      return device;
    },
    onSuccess: (device) => {
      void refetchMyDevices();
      setFeedback({
        tone: device.status === 'approved' ? 'success' : 'info',
        text:
          device.status === 'approved'
            ? t('attendance.device.enrolledApproved')
            : t('attendance.device.enrolledPending'),
      });
    },
    onError: (err) => {
      setFeedback({ tone: 'error', text: getErrorMessage(err, t('attendance.device.enrollFailed')) });
    },
  });

  const resolveCoords = useCallback(async (allowWarm: boolean): Promise<GeolocationCoordinates | null> => {
    if (allowWarm && warmCoordsRef.current) {
      const warm = await warmCoordsRef.current;
      lastGeoErrorRef.current = warm.errorCode;
      if (warm.coords) return warm.coords;
      warmCoordsRef.current = null;
    }
    const fresh = await getGeolocationResult(needsLocation);
    lastGeoErrorRef.current = fresh.errorCode;
    return fresh.coords;
  }, [needsLocation]);

  const buildCheckOptions = useCallback(
    async (purpose: 'check_in' | 'check_out', photoBase64?: string | null): Promise<AttendanceCheckOptions | undefined> => {
      let activeDeviceId = localDeviceId;
      if (
        !activeDeviceId &&
        deviceMode !== 'none' &&
        method?.device_auto_approve_first &&
        myDevices.filter((d) => d.status !== 'revoked').length === 0 &&
        isDeviceKeySupported()
      ) {
        try {
          const { publicKeyRawB64Url } = await getOrCreateDeviceKey();
          const enrolled = await enrollAttendanceDevice({
            public_key_raw: publicKeyRawB64Url,
            device_label: detectDefaultDeviceLabel(),
          });
          await setEnrolledDeviceId(enrolled.id, scopeKey);
          setLocalDeviceId(enrolled.id);
          activeDeviceId = enrolled.id;
          void refetchMyDevices();
        } catch {
          // Proceed cleanly; backend enforces policy if required
        }
      }

      let deviceProof;
      if (activeDeviceId && deviceMode !== 'none') {
        try {
          const challenge = await issueDeviceChallenge(purpose);
          const signed = await signDeviceChallenge(challenge, purpose, scopeKey, activeDeviceId);
          if (signed) deviceProof = signed;
        } catch {
          // Server will evaluate presence/absence of device_proof per policy
        }
      }

      if (!deviceProof && !photoBase64) return undefined;
      return {
        ...(deviceProof ? { device_proof: deviceProof } : {}),
        ...(photoBase64 ? { photo_base64: photoBase64 } : {}),
      };
    },
    [deviceMode, localDeviceId, method?.device_auto_approve_first, myDevices, refetchMyDevices, scopeKey],
  );

  const faceIdMut = useMutation({
    mutationFn: async (params: {
      challengeId: string;
      credential: AuthenticationResponseJSON;
      photoBase64?: string | null;
    }) => {
      const [coords, opts] = await Promise.all([
        resolveCoords(false),
        buildCheckOptions('check_in', params.photoBase64),
      ]);
      return opts
        ? checkInFaceId(params.challengeId, params.credential, coords?.latitude, coords?.longitude, opts)
        : checkInFaceId(params.challengeId, params.credential, coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => {
      setPendingPhoto(null);
      succeed(t('attendance.checkin.checkedInAt', { time: formatZonedTime(rec.check_in_at, tz) }));
    },
    onError: (err) => {
      setPendingPhoto(null);
      buzz(120);
      setFeedback(explain(err, t('attendance.checkin.faceIdFailed')));
    },
  });

  const qrMut = useMutation({
    mutationFn: async ({ token, photoBase64 }: { token: string; photoBase64?: string | null }) => {
      const [coords, opts] = await Promise.all([
        resolveCoords(true),
        buildCheckOptions('check_in', photoBase64),
      ]);
      return opts
        ? checkInQr(token, coords?.latitude, coords?.longitude, opts)
        : checkInQr(token, coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => {
      setScanning(false);
      setPendingPhoto(null);
      setScanErrorText('');
      succeed(t('attendance.checkin.checkedInAt', { time: formatZonedTime(rec.check_in_at, tz) }));
    },
    onError: (err) => {
      setPendingPhoto(null);
      buzz(120);
      const detail = explain(err, t('attendance.scan.checkInFailed'));
      if (detail.tone === 'info') {
        setScanning(false);
        setScanErrorText('');
        setFeedback(detail);
      } else {
        setScanErrorText(detail.text);
        setFeedback(detail);
      }
    },
  });

  const qrFaceIdMut = useMutation({
    mutationFn: async (params: {
      token: string;
      challengeId: string;
      credential: AuthenticationResponseJSON;
      photoBase64?: string | null;
    }) => {
      const [coords, opts] = await Promise.all([
        resolveCoords(true),
        buildCheckOptions('check_in', params.photoBase64),
      ]);
      return checkInQrFaceId(
        params.token,
        params.challengeId,
        params.credential,
        coords?.latitude,
        coords?.longitude,
        opts,
      );
    },
    onSuccess: (rec) => {
      setPendingQrToken(null);
      setPendingPhoto(null);
      succeed(t('attendance.checkin.checkedInAt', { time: formatZonedTime(rec.check_in_at, tz) }));
    },
    onError: (err) => {
      setPendingQrToken(null);
      setPendingPhoto(null);
      buzz(120);
      setFeedback(explain(err, t('attendance.checkin.faceIdFailed')));
    },
  });

  const checkOutMut = useMutation({
    mutationFn: async (photoBase64?: string | null) => {
      const [coords, opts] = await Promise.all([
        resolveCoords(false),
        buildCheckOptions('check_out', photoBase64),
      ]);
      return opts
        ? checkOut(coords?.latitude, coords?.longitude, opts)
        : checkOut(coords?.latitude, coords?.longitude);
    },
    onSuccess: (rec) => {
      setConfirmingOut(false);
      setPendingPhoto(null);
      buzz([18, 40, 18]);
      setFeedback({ tone: 'success', text: t('attendance.checkin.checkedOutAt', { time: formatZonedTime(rec.check_out_at, tz) }) });
      refreshAttendance();
    },
    onError: (err) => {
      setConfirmingOut(false);
      setPendingPhoto(null);
      buzz(120);
      setFeedback(explain(err, t('attendance.checkin.checkOutFailed')));
    },
  });

  const [ceremonyBusy, setCeremonyBusy] = useState(false);

  const runFaceIdCeremony = async (qrToken?: string) => {
    setCeremonyBusy(true);
    try {
      const { challenge_id, options } = await beginFaceIdCheckIn();
      const credential = await getPasskeyCredential(options.publicKey);
      if (photoMode !== 'none') {
        setPendingQrToken(null);
        setPendingPhoto(
          qrToken
            ? { kind: 'qr_face_id', token: qrToken, challengeId: challenge_id, credential }
            : { kind: 'face_id', challengeId: challenge_id, credential },
        );
      } else if (qrToken) {
        qrFaceIdMut.mutate({ token: qrToken, challengeId: challenge_id, credential });
      } else {
        faceIdMut.mutate({ challengeId: challenge_id, credential });
      }
    } catch (err) {
      buzz(120);
      setPendingQrToken(null);
      setFeedback(explain(err, t('attendance.checkin.faceIdFailed')));
    } finally {
      setCeremonyBusy(false);
    }
  };

  const handlePhotoConfirmed = (photoBase64: string | null) => {
    if (!pendingPhoto) return;
    if (pendingPhoto.kind === 'qr') {
      qrMut.mutate({ token: pendingPhoto.token, photoBase64 });
    } else if (pendingPhoto.kind === 'face_id') {
      faceIdMut.mutate({
        challengeId: pendingPhoto.challengeId,
        credential: pendingPhoto.credential,
        photoBase64,
      });
    } else if (pendingPhoto.kind === 'qr_face_id') {
      qrFaceIdMut.mutate({
        token: pendingPhoto.token,
        challengeId: pendingPhoto.challengeId,
        credential: pendingPhoto.credential,
        photoBase64,
      });
    } else if (pendingPhoto.kind === 'check_out') {
      checkOutMut.mutate(photoBase64);
    }
  };

  const submitting =
    ceremonyBusy ||
    faceIdMut.isPending ||
    qrMut.isPending ||
    qrFaceIdMut.isPending ||
    checkOutMut.isPending ||
    enrollMut.isPending;

  const handleCheckIn = () => {
    setFeedback(null);
    lastGeoErrorRef.current = null;
    if (!method) return;
    if (method.method === 'face_id') {
      void runFaceIdCeremony();
    } else {
      warmCoordsRef.current = null;
      setScanErrorText('');
      setScanning(true);
    }
  };

  const handleCheckOutClick = () => {
    if (!confirmingOut) {
      setConfirmingOut(true);
      return;
    }
    if (photoMode !== 'none') {
      setPendingPhoto({ kind: 'check_out' });
    } else {
      checkOutMut.mutate(null);
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
  if (todayFailed || methodFailed) {
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
        <button
          onClick={() => {
            if (todayFailed) void refetchToday();
            if (methodFailed) void refetchMethod();
          }}
          className="w-full min-h-12 rounded-lg bg-white hover:bg-zinc-100 text-[#1B2E4B] font-semibold flex items-center justify-center gap-2 active:scale-[0.99] transition-colors"
        >
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
          <div className="mt-4 flex items-start gap-2 rounded-lg bg-amber-400/15 border border-amber-300/30 px-3 py-2.5 text-sm text-amber-100">
            <TriangleAlert className="w-4 h-4 mt-0.5 shrink-0" />
            <span>{t('attendance.checkin.absentNotice')}</span>
          </div>
        )}

        {/* Inline persistent error/info feedback on the card */}
        {feedback && feedback.tone === 'error' && (
          <div
            role="alert"
            className="mt-4 flex items-start gap-2.5 rounded-lg bg-red-950/60 border border-red-300/30 px-3.5 py-3 text-sm text-red-100"
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
        <div className="mt-5 space-y-2.5">
          {state === 'checked-in' ? (
            <button
              onClick={handleCheckOutClick}
              disabled={submitting}
              className={`w-full min-h-12 rounded-lg font-semibold flex items-center justify-center gap-2.5 transition-colors active:scale-[0.99] disabled:opacity-60 ${
                confirmingOut ? 'bg-amber-300 text-amber-950' : 'bg-white/10 text-white border border-white/20 hover:bg-white/15'
              }`}
            >
              {checkOutMut.isPending
                ? <><Spinner /> {t('attendance.checkin.checkingOut')}</>
                : confirmingOut
                  ? <><LogOut className="w-5 h-5" /> {t('attendance.checkin.tapToConfirm')}</>
                  : <><LogOut className="w-5 h-5" /> {t('attendance.checkin.checkOut')}</>}
            </button>
          ) : state === 'checked-out' ? (
            <div className="w-full min-h-12 rounded-lg bg-white/10 border border-white/15 flex items-center justify-center gap-2 text-sm font-medium text-white/90">
              <CheckCircle2 className="w-5 h-5 text-emerald-300" /> {t('attendance.checkin.allDone')}
            </div>
          ) : (
            <button
              onClick={handleCheckIn}
              disabled={submitting || todayLoading || !method}
              className="w-full min-h-12 rounded-lg bg-white hover:bg-zinc-100 text-[#1B2E4B] text-base font-semibold flex items-center justify-center gap-2.5 transition-colors active:scale-[0.99] disabled:opacity-60 disabled:active:scale-100"
            >
              {faceIdMut.isPending || qrFaceIdMut.isPending || ceremonyBusy
                ? <><Spinner dark /> {t('attendance.checkin.checkingIn')}</>
                : !method || todayLoading
                  ? <><Spinner dark /> {t('common.loading')}</>
                  : method.method === 'face_id'
                    ? <><Fingerprint className="w-5 h-5" /> {t('attendance.checkin.checkInFaceId')}</>
                    : method.method === 'qr_and_face_id'
                      ? <><ScanLine className="w-5 h-5" /> {t('attendance.checkin.checkInQrFaceId')}</>
                      : <><ScanLine className="w-5 h-5" /> {t('attendance.checkin.checkIn')}</>}
            </button>
          )}

          {method && state !== 'checked-out' && (
            <button
              type="button"
              onClick={() => setKioskPassOpen(true)}
              disabled={submitting}
              className="w-full min-h-10 rounded-lg bg-white/10 hover:bg-white/15 border border-white/15 text-xs font-medium text-white/90 flex items-center justify-center gap-2 transition-colors disabled:opacity-50"
            >
              <QrCode className="w-3.5 h-3.5" />
              {state === 'checked-in'
                ? t('attendance.kioskPass.showCheckOut')
                : t('attendance.kioskPass.showCheckIn')}
            </button>
          )}
        </div>

        {/* Bound device & verification hints */}
        {state !== 'checked-out' && (needsLocation || showWifiHint || deviceMode !== 'none' || photoMode !== 'none') && (
          <div className="mt-3 space-y-2">
            {(needsLocation || showWifiHint || photoMode !== 'none') && (
              <div className="flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-[11px] text-white/65">
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
                {photoMode !== 'none' && (
                  <span className="flex items-center gap-1.5">
                    <Camera className="w-3 h-3 shrink-0" />
                    {photoMode === 'required'
                      ? t('attendance.photo.requiredHint')
                      : t('attendance.photo.optionalHint')}
                  </span>
                )}
              </div>
            )}

            {deviceMode !== 'none' && isDeviceKeySupported() && (
              <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl bg-black/20 border border-white/10 px-3 py-2 text-xs">
                {matchedDevice?.status === 'approved' ? (
                  <span className="inline-flex items-center gap-1.5 text-emerald-200">
                    <ShieldCheck className="w-3.5 h-3.5 shrink-0" />
                    {t('attendance.device.trustedBadge', { label: matchedDevice.device_label })}
                  </span>
                ) : matchedDevice?.status === 'pending' ? (
                  <span className="inline-flex items-center gap-1.5 text-amber-200">
                    <ShieldAlert className="w-3.5 h-3.5 shrink-0" />
                    {t('attendance.device.pendingBadge')}
                  </span>
                ) : matchedDevice?.status === 'revoked' ? (
                  <>
                    <span className="inline-flex items-center gap-1.5 text-red-200">
                      <ShieldAlert className="w-3.5 h-3.5 shrink-0" />
                      {t('attendance.device.revokedBadge')}
                    </span>
                    <button
                      type="button"
                      onClick={() => enrollMut.mutate(true)}
                      disabled={enrollMut.isPending}
                      className="font-semibold text-white underline underline-offset-2"
                    >
                      {t('attendance.device.reEnrollButton')}
                    </button>
                  </>
                ) : (
                  <>
                    <span className="inline-flex items-center gap-1.5 text-white/80">
                      <Smartphone className="w-3.5 h-3.5 shrink-0" />
                      {deviceMode === 'enforce'
                        ? t('attendance.device.unboundEnforced')
                        : t('attendance.device.unboundWarn')}
                    </span>
                    <button
                      type="button"
                      onClick={() => enrollMut.mutate(false)}
                      disabled={enrollMut.isPending}
                      className="font-semibold text-white underline underline-offset-2"
                    >
                      {t('attendance.device.enrollButton')}
                    </button>
                  </>
                )}
              </div>
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
          <div className={`flex items-start gap-2.5 rounded-lg border border-black/10 px-4 py-3 text-sm text-white shadow-md ${
            feedback.tone === 'success' ? 'bg-emerald-700' : 'bg-[#1B2E4B]'
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
          busy={qrMut.isPending || qrFaceIdMut.isPending}
          errorText={scanErrorText}
          onRetryScan={() => setScanErrorText('')}
          onClose={() => { setScanning(false); setScanErrorText(''); warmCoordsRef.current = null; }}
          onCameraReady={() => {
            if (needsLocation) {
              warmCoordsRef.current = getGeolocationResult(true);
            }
          }}
          onToken={(token) => {
            setScanErrorText('');
            if (method?.method === 'qr_and_face_id') {
              setScanning(false);
              setPendingQrToken(token);
            } else if (photoMode !== 'none') {
              setScanning(false);
              setPendingPhoto({ kind: 'qr', token });
            } else {
              qrMut.mutate({ token });
            }
          }}
        />
      )}

      <Modal
        open={Boolean(pendingQrToken)}
        onClose={() => !submitting && setPendingQrToken(null)}
        title={t('attendance.checkin.qrFaceIdStepTitle')}
        maxWidth="max-w-sm"
      >
        <div className="space-y-4 text-center">
          <div className="mx-auto w-12 h-12 rounded-lg bg-[var(--accent-soft)] flex items-center justify-center">
            <Fingerprint className="w-6 h-6 text-[var(--accent-1)]" />
          </div>
          <p className="text-sm text-[var(--muted-foreground)]">
            {t('attendance.checkin.qrFaceIdStepBody')}
          </p>
          <div className="flex flex-col gap-2">
            <button
              type="button"
              disabled={submitting || !pendingQrToken}
              onClick={() => pendingQrToken && void runFaceIdCeremony(pendingQrToken)}
              className="btn-primary w-full justify-center py-2.5"
            >
              <Fingerprint className="w-4 h-4" />
              {submitting ? t('attendance.checkin.checkingIn') : t('attendance.checkin.confirmFaceIdButton')}
            </button>
            <button
              type="button"
              disabled={submitting}
              onClick={() => setPendingQrToken(null)}
              className="btn-secondary w-full justify-center"
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      </Modal>

      <LivePhotoCaptureModal
        open={Boolean(pendingPhoto)}
        title={
          pendingPhoto?.kind === 'check_out'
            ? t('attendance.photo.checkOutTitle')
            : t('attendance.photo.checkInTitle')
        }
        description={
          photoMode === 'required'
            ? t('attendance.photo.requiredDescription')
            : t('attendance.photo.optionalDescription')
        }
        required={photoMode === 'required'}
        submitting={submitting}
        onConfirm={handlePhotoConfirmed}
        onCancel={() => {
          setPendingPhoto(null);
          setConfirmingOut(false);
        }}
      />

      <PersonalKioskPassModal
        open={kioskPassOpen}
        onClose={() => setKioskPassOpen(false)}
        hasOpenSession={state === 'checked-in'}
        deviceMode={deviceMode}
        scopeKey={scopeKey}
      />
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
    method?.method === 'face_id'
      ? t('attendance.checkin.faceId')
      : method?.method === 'qr_and_face_id'
        ? t('attendance.checkin.qrAndFaceId')
        : t('attendance.checkin.qrCode');
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
                  {record?.hours_worked != null && Number.isFinite(Number(record.hours_worked)) && (
                    <> · {t('attendance.checkin.hoursShort', { count: Number(record.hours_worked) })}</>
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

/** Brand navy card shell so white ink maintains crisp contrast without decorative gradients. */
function Shell({ children }: { children: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-white/10 bg-[#1B2E4B] dark:bg-[#141417] text-white p-5 sm:p-6">
      {children}
    </section>
  );
}

function Spinner({ dark }: { dark?: boolean }) {
  return (
    <span
      aria-hidden
      className={`inline-block h-4 w-4 animate-spin rounded-full border-2 border-transparent ${
        dark ? 'border-t-[#1B2E4B] border-r-[#1B2E4B]/40' : 'border-t-white border-r-white/40'
      }`}
    />
  );
}
