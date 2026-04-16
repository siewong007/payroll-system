import { useState, useEffect, useRef, useCallback } from 'react';
import { RefreshCw, Wifi, Clock } from 'lucide-react';
import QRCode from 'qrcode';
import { generateQrToken } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';

const QR_TTL = 60; // seconds

export function AttendanceKiosk() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [timeLeft, setTimeLeft] = useState(QR_TTL);
  const [scanUrl, setScanUrl] = useState('');
  const [expiresAt, setExpiresAt] = useState<Date | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [now, setNow] = useState(new Date());
  
  const { isAuthenticated, isLoading: authLoading } = useAuth();

  // Live clock
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  const fetchToken = useCallback(async () => {
    try {
      setLoading(true);
      setError('');
      const data = await generateQrToken();
      setScanUrl(data.scan_url);
      setExpiresAt(new Date(data.expires_at));
    } catch {
      setError('Failed to load QR code. Retrying…');
      setTimeout(fetchToken, 5000);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (authLoading) return;
    if (!isAuthenticated) {
      window.location.href = '/login';
      return;
    }
    fetchToken();
  }, [fetchToken, authLoading, isAuthenticated]);

  // Draw QR whenever scanUrl changes
  useEffect(() => {
    if (!scanUrl || !canvasRef.current) return;
    QRCode.toCanvas(canvasRef.current, scanUrl, {
      width: 400,
      margin: 1,
      errorCorrectionLevel: 'L',
      color: { dark: '#111827', light: '#ffffff' },
    });
  }, [scanUrl]);

  // Countdown timer
  useEffect(() => {
    if (!expiresAt) return;
    const tick = () => {
      const left = Math.max(0, Math.floor((expiresAt.getTime() - Date.now()) / 1000));
      setTimeLeft(left);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [expiresAt]);

  // Auto-refresh when expired
  useEffect(() => {
    if (timeLeft === 0 && !loading && !authLoading && isAuthenticated) {
      fetchToken();
    }
  }, [timeLeft, loading, authLoading, isAuthenticated, fetchToken]);

  const pctLeft = Math.max(0, (timeLeft / QR_TTL) * 100);
  const isLow = timeLeft <= 10;

  const timeStr = now.toLocaleTimeString('en-MY', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const dateStr = now.toLocaleDateString('en-MY', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

  return (
    <div className="min-h-screen bg-gray-950 flex flex-col items-center justify-center p-6 select-none">
      {/* Header */}
      <div className="mb-8 text-center">
        <div className="flex items-center justify-center gap-3 mb-2">
          <div className="w-10 h-10 bg-white rounded-xl flex items-center justify-center">
            <span className="font-black text-gray-950 text-xl">P</span>
          </div>
          <h1 className="text-3xl font-black text-white tracking-tight">PayrollMY</h1>
        </div>
        <p className="text-gray-400 text-sm font-medium uppercase tracking-wider">Attendance Kiosk</p>
      </div>

      {/* Main Card */}
      <div className="bg-gray-900 border border-gray-800 rounded-3xl p-8 flex flex-col items-center gap-6 w-full max-w-sm shadow-2xl">

        {/* Date / Time */}
        <div className="text-center">
          <div className="text-5xl font-black text-white tabular-nums tracking-tight">{timeStr}</div>
          <div className="text-gray-400 text-sm mt-1">{dateStr}</div>
        </div>

        {/* QR Code */}
        <div className="relative">
          {loading ? (
            <div className="w-[320px] h-[320px] flex items-center justify-center bg-gray-800 rounded-2xl">
              <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-white" />
            </div>
          ) : error ? (
            <div className="w-[320px] h-[320px] flex flex-col items-center justify-center bg-red-950/50 border border-red-500/30 rounded-2xl gap-3">
              <Wifi className="w-10 h-10 text-red-400" />
              <p className="text-red-400 text-sm text-center px-4">{error}</p>
            </div>
          ) : (
            <div className="bg-white p-4 rounded-2xl shadow-[0_0_50px_rgba(255,255,255,0.15)] transition-transform hover:scale-[1.02]">
              <canvas ref={canvasRef} className="block rounded-lg shadow-sm" />
            </div>
          )}
        </div>

        {/* Countdown */}
        {!loading && !error && (
          <div className="w-full">
            <div className="flex items-center justify-between text-xs mb-2">
              <div className="flex items-center gap-1.5 text-gray-500">
                <RefreshCw className="w-3 h-3" />
                Refreshes in
              </div>
              <span className={`font-bold tabular-nums text-base ${isLow ? 'text-red-400 animate-pulse' : 'text-white'}`}>
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

        {/* Instructions */}
        <div className="w-full bg-gray-800/60 rounded-2xl p-4 space-y-2">
          {[
            '1. Open PayrollMY on your phone',
            '2. Tap "Attendance" in the portal',
            '3. Scan the QR code above',
            '4. Confirm your location',
          ].map(step => (
            <div key={step} className="text-gray-300 text-sm flex items-start gap-2">
              <span className="text-emerald-400 shrink-0">›</span>
              <span>{step}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Footer */}
      <div className="mt-8 flex items-center gap-2 text-gray-600 text-xs">
        <Clock className="w-3.5 h-3.5" />
        <span>QR code regenerates automatically every {QR_TTL} seconds</span>
      </div>
    </div>
  );
}
