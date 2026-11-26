import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { RefreshCw, Wifi, Clock, ShieldAlert } from 'lucide-react';
import QRCode from 'qrcode';
import { fetchKioskQr } from '@/api/kiosk';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { formatTimeSeconds, formatDateLong } from '@/lib/format';

type Status =
  | { kind: 'loading' }
  | { kind: 'ok'; scanUrl: string; ttl: number; startedAt: number }
  | { kind: 'transient'; message: string }
  | { kind: 'fatal'; message: string };

const FALLBACK_TTL = 60;
const MAX_BACKOFF_MS = 60_000;
const KIOSK_SECRET_STORAGE_KEY = 'kiosk_secret_v1';

/** Re-mint this many seconds before the server deadline, covering request latency. */
const GUARD_SECONDS = 5;

/**
 * The floor no clock, effect or future refactor can get under: two mints can
 * never happen within this window. Everything else here is a correctness fix;
 * this is the containment. A tablet whose clock ran fast used to read every
 * freshly minted token as already expired and mint again immediately, and
 * because the rate limiters key on the forwarded client IP, one such tablet
 * drained the bucket for every kiosk behind that address.
 */
const MIN_MINT_INTERVAL_MS = 5_000;

export function AttendanceKioskPublic() {
  const { t } = useTranslation();
  const { kioskKey: paramKey = '' } = useParams<{ kioskKey: string }>();
  const [kioskKey] = useState<string>(() => {
    const fromHash = window.location.hash.replace(/^#/, '').trim();
    if (fromHash) {
      try {
        window.sessionStorage.setItem(KIOSK_SECRET_STORAGE_KEY, fromHash);
      } catch {
        /* ignore quota / privacy mode failures */
      }
      return fromHash;
    }
    if (paramKey) {
      try {
        window.sessionStorage.setItem(KIOSK_SECRET_STORAGE_KEY, paramKey);
      } catch {
        /* ignore quota / privacy mode failures */
      }
      return paramKey;
    }
    try {
      return window.sessionStorage.getItem(KIOSK_SECRET_STORAGE_KEY) ?? '';
    } catch {
      return '';
    }
  });
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Immediately scrub any kiosk secret from the address bar / browser history
  // (both `#secret` fragments and legacy `/kiosk/:kioskKey` URLs).
  useEffect(() => {
    const legacyPath =
      window.location.pathname.startsWith('/kiosk/') &&
      window.location.pathname.length > '/kiosk/'.length;
    if (window.location.hash || legacyPath) {
      window.history.replaceState(null, '', '/kiosk');
    }
  }, []);
  const [status, setStatus] = useState<Status>({ kind: 'loading' });
  const [timeLeft, setTimeLeft] = useState(FALLBACK_TTL);
  const [retrying, setRetrying] = useState(false);
  const backoffRef = useRef(2000);
  const lastMintRef = useRef(Number.NEGATIVE_INFINITY);
  const mintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const statusKindRef = useRef<Status['kind']>('loading');
  const nextRefreshAtRef = useRef(0);
  const fetchTokenRef = useRef<() => Promise<void>>(() => Promise.resolve());

  useEffect(() => {
    statusKindRef.current = status.kind;
    if (status.kind === 'ok') {
      nextRefreshAtRef.current = status.startedAt + Math.max(1, status.ttl - GUARD_SECONDS) * 1000;
    }
  }, [status]);

  // Block search engines and Referer leakage of the secret in the URL.
  useEffect(() => {
    const m1 = document.createElement('meta');
    m1.name = 'robots';
    m1.content = 'noindex, nofollow';
    document.head.appendChild(m1);
    const m2 = document.createElement('meta');
    m2.name = 'referrer';
    m2.content = 'no-referrer';
    document.head.appendChild(m2);
    return () => {
      document.head.removeChild(m1);
      document.head.removeChild(m2);
    };
  }, []);

  const mountedRef = useRef(true);
  // One timer for every scheduled mint, so a reschedule replaces the pending one
  // instead of stacking a second alongside it.
  const scheduleMint = useCallback((delayMs: number) => {
    if (!mountedRef.current) return;
    if (mintTimerRef.current !== null) {
      clearTimeout(mintTimerRef.current);
    }
    mintTimerRef.current = setTimeout(() => {
      mintTimerRef.current = null;
      if (mountedRef.current) {
        void fetchTokenRef.current();
      }
    }, Math.max(0, delayMs));
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (mintTimerRef.current !== null) {
        clearTimeout(mintTimerRef.current);
      }
    };
  }, []);

  const fetchToken = useCallback(async () => {
    if (!mountedRef.current) return;
    // Defer rather than drop: the backoff retry after a network error asks for
    // 2 s, and silently swallowing it would strand the kiosk on its error card.
    const sinceLastMint = performance.now() - lastMintRef.current;
    if (sinceLastMint < MIN_MINT_INTERVAL_MS) {
      scheduleMint(MIN_MINT_INTERVAL_MS - sinceLastMint);
      return;
    }
    lastMintRef.current = performance.now();

    try {
      const data = await fetchKioskQr(kioskKey);
      if (!mountedRef.current) return;
      backoffRef.current = 2000;
      setRetrying(false);
      const ttl = data.ttl_seconds > 0 ? data.ttl_seconds : FALLBACK_TTL;
      setStatus({
        kind: 'ok',
        scanUrl: data.scan_url,
        ttl,
        startedAt: performance.now(),
      });
      scheduleMint(Math.max(MIN_MINT_INTERVAL_MS, (ttl - GUARD_SECONDS) * 1000));
    } catch (err: unknown) {
      if (!mountedRef.current) return;
      setRetrying(false);
      const e = err as { response?: { status?: number } };
      const code = e.response?.status;
      if (code === 401) {
        try {
          window.sessionStorage.removeItem(KIOSK_SECRET_STORAGE_KEY);
        } catch {
          /* ignore */
        }
        setStatus({
          kind: 'fatal',
          message: t('attendance.kiosk.disconnected'),
        });
        return;
      }
      const wait = Math.min(MAX_BACKOFF_MS, backoffRef.current);
      backoffRef.current = Math.min(MAX_BACKOFF_MS, backoffRef.current * 2);
      const note =
        code === 429
          ? t('attendance.kiosk.rateLimited', { secs: Math.round(wait / 1000) })
          : t('attendance.kiosk.networkProblem', { secs: Math.round(wait / 1000) });
      setStatus({ kind: 'transient', message: note });
      scheduleMint(wait);
    }
  }, [kioskKey, scheduleMint, t]); // eslint-disable-line react-hooks/exhaustive-deps -- Babel cannot identify setStatus as a stable React setter.

  useEffect(() => {
    fetchTokenRef.current = fetchToken;
  }, [fetchToken]);

  useEffect(() => {
    if (!kioskKey) return;
    // Defer one microtask so the lint rule (and React Compiler) sees this as not
    // a synchronous render→setState chain.
    scheduleMint(0);
  }, [kioskKey, scheduleMint]);

  // Keep tablet screen awake and recover promptly on reconnect / wake-from-sleep.
  useEffect(() => {
    if (!kioskKey) return;
    let wakeLock: { release: () => Promise<void> } | null = null;
    let disposed = false;

    const acquireWakeLock = async () => {
      if (disposed || typeof navigator === 'undefined') return;
      const wl = (navigator as Navigator & { wakeLock?: { request: (type: 'screen') => Promise<{ release: () => Promise<void> }> } }).wakeLock;
      if (!wl || document.visibilityState !== 'visible') return;
      try {
        const sentinel = await wl.request('screen');
        if (disposed) {
          void sentinel.release().catch(() => {});
          return;
        }
        wakeLock = sentinel;
      } catch {
        /* browser or power-saver refused Wake Lock — non-fatal */
      }
    };

    const handleVisibility = () => {
      if (document.visibilityState !== 'visible') return;
      void acquireWakeLock();
      const kind = statusKindRef.current;
      if (kind === 'transient' || (kind === 'ok' && performance.now() >= nextRefreshAtRef.current)) {
        backoffRef.current = 2000;
        scheduleMint(0);
      }
    };

    const handleOnline = () => {
      if (statusKindRef.current === 'transient') {
        backoffRef.current = 2000;
        scheduleMint(0);
      }
    };

    void acquireWakeLock();
    document.addEventListener('visibilitychange', handleVisibility);
    window.addEventListener('online', handleOnline);

    return () => {
      disposed = true;
      document.removeEventListener('visibilitychange', handleVisibility);
      window.removeEventListener('online', handleOnline);
      if (wakeLock) {
        void wakeLock.release().catch(() => { /* ignore */ });
      }
    };
  }, [kioskKey, scheduleMint]);

  // Draw QR whenever scanUrl changes
  useEffect(() => {
    if (status.kind !== 'ok' || !canvasRef.current) return;
    QRCode.toCanvas(canvasRef.current, status.scanUrl, {
      width: 400,
      margin: 2,
      errorCorrectionLevel: 'M',
      color: { dark: '#111827', light: '#ffffff' },
    });
  }, [status]);

  // Countdown. Both terms come from the same monotonic local clock, so a device
  // whose wall clock is minutes off the server still shows a full TTL.
  useEffect(() => {
    if (status.kind !== 'ok') return;
    const { ttl, startedAt } = status;
    const tick = () => {
      setTimeLeft(Math.max(0, Math.ceil(ttl - (performance.now() - startedAt) / 1000)));
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [status]);

  const effectiveStatus: Status = !kioskKey
    ? { kind: 'fatal', message: t('attendance.kiosk.missingCredential') }
    : status;
  const ttl = effectiveStatus.kind === 'ok' ? effectiveStatus.ttl : FALLBACK_TTL;
  const pctLeft = ttl > 0 ? Math.max(0, (timeLeft / ttl) * 100) : 0;
  const isLow = timeLeft <= 10;

  return (
    <main className="min-h-[100dvh] safe-area-top safe-area-bottom bg-gray-950 text-white flex flex-col items-center justify-center p-4 sm:p-6 select-none">
      <div className="w-full max-w-md sm:max-w-lg md:max-w-xl lg:max-w-4xl bg-gray-900 border border-gray-800 rounded-3xl p-6 sm:p-8 shadow-2xl grid grid-cols-1 lg:grid-cols-2 lg:items-center gap-6 lg:gap-10">
        {/* Info / Clock column (left on landscape, split cleanly below QR on portrait) */}
        <div className="flex flex-col items-center lg:items-start text-center lg:text-left gap-6">
          <div>
            <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto lg:mx-0 mb-2 rounded-xl" />
            <h1 className="text-gray-300 text-sm font-medium uppercase tracking-wider">
              {t('attendance.kiosk.title')}
            </h1>
          </div>

          <KioskWallClock alignedLeftOnLarge />

          <ol className="hidden lg:block w-full bg-gray-800/70 border border-gray-700/60 rounded-2xl p-4 space-y-2 text-left">
            {[1, 2, 3, 4].map((step) => (
              <li key={step} className="text-gray-200 text-sm flex items-start gap-2">
                <span className="text-emerald-400 shrink-0" aria-hidden="true">›</span>
                <span>{t(`attendance.kiosk.steps.${step}`)}</span>
              </li>
            ))}
          </ol>
        </div>

        {/* QR column */}
        <div className="flex flex-col items-center gap-5 w-full">
          <div className="relative w-full flex justify-center">
            {effectiveStatus.kind === 'loading' && (
              <div
                role="status"
                aria-label={t('common.loading')}
                className="w-full max-w-[320px] md:max-w-[360px] aspect-square flex flex-col items-center justify-center gap-3 bg-gray-800 rounded-2xl"
              >
                <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-white" />
                <span className="sr-only">{t('common.loading')}</span>
              </div>
            )}
            {effectiveStatus.kind === 'transient' && (
              <div
                role="alert"
                className="w-full max-w-[320px] md:max-w-[360px] aspect-square flex flex-col items-center justify-center bg-amber-950/40 border border-amber-500/30 rounded-2xl gap-3 px-6 text-center"
              >
                <Wifi className="w-10 h-10 text-amber-400" aria-hidden="true" />
                <p className="text-amber-200 text-sm">{effectiveStatus.message}</p>
                <button
                  type="button"
                  disabled={retrying}
                  onClick={() => {
                    setRetrying(true);
                    backoffRef.current = 2000;
                    scheduleMint(0);
                  }}
                  className="mt-1 min-h-11 inline-flex items-center gap-2 rounded-xl bg-amber-500/20 hover:bg-amber-500/30 border border-amber-400/40 px-4 py-2.5 text-sm font-semibold text-amber-100 transition-colors disabled:opacity-60"
                >
                  <RefreshCw className={`w-4 h-4 ${retrying ? 'animate-spin' : ''}`} aria-hidden="true" />
                  {t('attendance.kiosk.retryNow')}
                </button>
              </div>
            )}
            {effectiveStatus.kind === 'fatal' && (
              <div
                role="alert"
                className="w-full max-w-[320px] md:max-w-[360px] aspect-square flex flex-col items-center justify-center bg-red-950/50 border border-red-500/30 rounded-2xl gap-3 px-6 text-center"
              >
                <ShieldAlert className="w-10 h-10 text-red-400" aria-hidden="true" />
                <p className="text-red-200 text-sm">{effectiveStatus.message}</p>
              </div>
            )}
            {effectiveStatus.kind === 'ok' && (
              <div className="relative w-full max-w-[320px] md:max-w-[360px] lg:max-w-[380px] bg-white p-3 sm:p-4 rounded-2xl shadow-[0_0_50px_rgba(255,255,255,0.15)]">
                <canvas
                  ref={canvasRef}
                  role="img"
                  aria-label={t('attendance.kiosk.title')}
                  className={`block w-full !h-auto aspect-square rounded-lg shadow-sm transition-opacity ${
                    timeLeft === 0 ? 'opacity-30' : 'opacity-100'
                  }`}
                />
                {timeLeft === 0 && (
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div
                      role="status"
                      className="rounded-xl bg-gray-900/90 border border-gray-700 px-4 py-2 text-sm font-semibold text-white shadow-lg"
                    >
                      {t('attendance.qrPanel.refreshing')}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          {effectiveStatus.kind === 'ok' && (
            <div className="w-full max-w-[320px] md:max-w-[360px] lg:max-w-[380px]">
              <div className="flex items-center justify-between text-xs mb-2">
                <div className="flex items-center gap-1.5 text-gray-300">
                  <RefreshCw className="w-3 h-3" aria-hidden="true" />
                  {t('attendance.kiosk.refreshesIn')}
                </div>
                <span
                  className={`font-bold tabular-nums text-base ${isLow ? 'text-red-400 motion-safe:animate-pulse' : 'text-white'}`}
                >
                  {t('attendance.kiosk.secondsShort', { count: timeLeft })}
                </span>
              </div>
              <div
                role="progressbar"
                aria-label={t('attendance.kiosk.refreshesIn')}
                aria-valuemin={0}
                aria-valuemax={ttl}
                aria-valuenow={timeLeft}
                className="h-2 bg-gray-800 rounded-full overflow-hidden"
              >
                <div
                  className={`h-full rounded-full transition-all duration-1000 ${isLow ? 'bg-red-500' : 'bg-emerald-500'}`}
                  style={{ width: `${pctLeft}%` }}
                />
              </div>
            </div>
          )}

          {/* Instructions sit below QR on portrait tablets/phones so the QR code stays prominent */}
          <ol className="lg:hidden w-full bg-gray-800/70 border border-gray-700/60 rounded-2xl p-4 space-y-2 text-left">
            {[1, 2, 3, 4].map((step) => (
              <li key={step} className="text-gray-200 text-sm flex items-start gap-2">
                <span className="text-emerald-400 shrink-0" aria-hidden="true">›</span>
                <span>{t(`attendance.kiosk.steps.${step}`)}</span>
              </li>
            ))}
          </ol>
        </div>
      </div>

      <div className="mt-6 flex items-center gap-2 text-gray-400 text-xs">
        <Clock className="w-3.5 h-3.5" aria-hidden="true" />
        <span>{t('attendance.kiosk.autoRegen')}</span>
      </div>
    </main>
  );
}

function KioskWallClock({ alignedLeftOnLarge = false }: { alignedLeftOnLarge?: boolean }) {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className={alignedLeftOnLarge ? 'text-center lg:text-left' : 'text-center'}>
      <div className="text-4xl sm:text-5xl font-black text-white tabular-nums tracking-tight">
        {formatTimeSeconds(now)}
      </div>
      <div className="text-gray-300 text-sm mt-1">{formatDateLong(now)}</div>
    </div>
  );
}
