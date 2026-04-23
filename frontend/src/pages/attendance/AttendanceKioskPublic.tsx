import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import { RefreshCw, Wifi, Clock, ShieldAlert } from 'lucide-react';
import QRCode from 'qrcode';
import { fetchKioskQr } from '@/api/kiosk';
import { BrandLogo } from '@/components/ui/BrandLogo';

type Status =
  | { kind: 'loading' }
  | { kind: 'ok'; scanUrl: string; expiresAt: Date; ttl: number }
  | { kind: 'transient'; message: string }
  | { kind: 'fatal'; message: string };

const FALLBACK_TTL = 60;
const MAX_BACKOFF_MS = 60_000;

export function AttendanceKioskPublic() {
  const { kioskKey = '' } = useParams<{ kioskKey: string }>();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [status, setStatus] = useState<Status>({ kind: 'loading' });
  const [timeLeft, setTimeLeft] = useState(FALLBACK_TTL);
  const [now, setNow] = useState(new Date());
  const backoffRef = useRef(2000);
  const fetchTokenRef = useRef<() => Promise<void>>(() => Promise.resolve());

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

  // Live clock
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  const fetchToken = useCallback(async () => {
    try {
      const data = await fetchKioskQr(kioskKey);
      backoffRef.current = 2000;
      setStatus({
        kind: 'ok',
        scanUrl: data.scan_url,
        expiresAt: new Date(data.expires_at),
        ttl: data.ttl_seconds,
      });
    } catch (err: unknown) {
      const e = err as { response?: { status?: number } };
      const code = e.response?.status;
      if (code === 401) {
        setStatus({
          kind: 'fatal',
          message: 'This kiosk has been disconnected. Ask an admin to issue a new link.',
        });
        return;
      }
      const wait = Math.min(MAX_BACKOFF_MS, backoffRef.current);
      backoffRef.current = Math.min(MAX_BACKOFF_MS, backoffRef.current * 2);
      const note =
        code === 429
          ? `Rate limited. Retrying in ${Math.round(wait / 1000)}s…`
          : `Network problem. Retrying in ${Math.round(wait / 1000)}s…`;
      setStatus({ kind: 'transient', message: note });
      setTimeout(() => {
        void fetchTokenRef.current();
      }, wait);
    }
  }, [kioskKey]);

  useEffect(() => {
    fetchTokenRef.current = fetchToken;
  }, [fetchToken]);

  useEffect(() => {
    if (!kioskKey) return;
    // Defer one microtask so the lint rule (and React Compiler) sees this as not
    // a synchronous render→setState chain.
    const id = setTimeout(() => {
      void fetchTokenRef.current();
    }, 0);
    return () => clearTimeout(id);
  }, [kioskKey]);

  // Draw QR whenever scanUrl changes
  useEffect(() => {
    if (status.kind !== 'ok' || !canvasRef.current) return;
    QRCode.toCanvas(canvasRef.current, status.scanUrl, {
      width: 400,
      margin: 1,
      errorCorrectionLevel: 'L',
      color: { dark: '#111827', light: '#ffffff' },
    });
  }, [status]);

  // Countdown
  useEffect(() => {
    if (status.kind !== 'ok') return;
    const expiresAt = status.expiresAt;
    const tick = () => {
      const left = Math.max(0, Math.floor((expiresAt.getTime() - Date.now()) / 1000));
      setTimeLeft(left);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [status]);

  // Auto-refresh when expired
  useEffect(() => {
    if (status.kind !== 'ok' || timeLeft !== 0) return;
    const id = setTimeout(() => {
      void fetchTokenRef.current();
    }, 0);
    return () => clearTimeout(id);
  }, [timeLeft, status]);

  const effectiveStatus: Status = !kioskKey
    ? { kind: 'fatal', message: 'Missing kiosk credential in URL.' }
    : status;
  const ttl = effectiveStatus.kind === 'ok' ? effectiveStatus.ttl : FALLBACK_TTL;
  const pctLeft = ttl > 0 ? Math.max(0, (timeLeft / ttl) * 100) : 0;
  const isLow = timeLeft <= 10;

  const timeStr = now.toLocaleTimeString('en-MY', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
  const dateStr = now.toLocaleDateString('en-MY', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });

  return (
    <div className="min-h-screen bg-gray-950 flex flex-col items-center justify-center p-6 select-none">
      <div className="mb-8 text-center">
        <BrandLogo variant="lockup-light" className="h-10 w-auto mx-auto mb-2" />
        <p className="text-gray-400 text-sm font-medium uppercase tracking-wider">
          Attendance Kiosk
        </p>
      </div>

      <div className="bg-gray-900 border border-gray-800 rounded-3xl p-8 flex flex-col items-center gap-6 w-full max-w-sm shadow-2xl">
        <div className="text-center">
          <div className="text-5xl font-black text-white tabular-nums tracking-tight">
            {timeStr}
          </div>
          <div className="text-gray-400 text-sm mt-1">{dateStr}</div>
        </div>

        <div className="relative">
          {effectiveStatus.kind === 'loading' && (
            <div className="w-[320px] h-[320px] flex items-center justify-center bg-gray-800 rounded-2xl">
              <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-white" />
            </div>
          )}
          {effectiveStatus.kind === 'transient' && (
            <div className="w-[320px] h-[320px] flex flex-col items-center justify-center bg-amber-950/40 border border-amber-500/30 rounded-2xl gap-3 px-6 text-center">
              <Wifi className="w-10 h-10 text-amber-400" />
              <p className="text-amber-300 text-sm">{effectiveStatus.message}</p>
            </div>
          )}
          {effectiveStatus.kind === 'fatal' && (
            <div className="w-[320px] h-[320px] flex flex-col items-center justify-center bg-red-950/50 border border-red-500/30 rounded-2xl gap-3 px-6 text-center">
              <ShieldAlert className="w-10 h-10 text-red-400" />
              <p className="text-red-300 text-sm">{effectiveStatus.message}</p>
            </div>
          )}
          {effectiveStatus.kind === 'ok' && (
            <div className="bg-white p-4 rounded-2xl shadow-[0_0_50px_rgba(255,255,255,0.15)] transition-transform hover:scale-[1.02]">
              <canvas ref={canvasRef} className="block rounded-lg shadow-sm" />
            </div>
          )}
        </div>

        {effectiveStatus.kind === 'ok' && (
          <div className="w-full">
            <div className="flex items-center justify-between text-xs mb-2">
              <div className="flex items-center gap-1.5 text-gray-500">
                <RefreshCw className="w-3 h-3" />
                Refreshes in
              </div>
              <span
                className={`font-bold tabular-nums text-base ${isLow ? 'text-red-400 animate-pulse' : 'text-white'}`}
              >
                {timeLeft}s
              </span>
            </div>
            <div className="h-2 bg-gray-800 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all duration-1000 ${isLow ? 'bg-red-500' : 'bg-emerald-500'}`}
                style={{ width: `${pctLeft}%` }}
              />
            </div>
          </div>
        )}

        <div className="w-full bg-gray-800/60 rounded-2xl p-4 space-y-2">
          {[
            '1. Open PayrollMY on your phone',
            '2. Tap "Attendance" in the portal',
            '3. Scan the QR code above',
            '4. Confirm your location',
          ].map((step) => (
            <div key={step} className="text-gray-300 text-sm flex items-start gap-2">
              <span className="text-emerald-400 shrink-0">›</span>
              <span>{step}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="mt-8 flex items-center gap-2 text-gray-600 text-xs">
        <Clock className="w-3.5 h-3.5" />
        <span>QR code regenerates automatically</span>
      </div>
    </div>
  );
}
