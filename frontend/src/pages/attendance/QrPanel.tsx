import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertCircle, Eye, EyeOff, Link2, QrCode, RefreshCw } from 'lucide-react';
import QRCode from 'qrcode';
import { generateQrToken } from '@/api/attendance';
import { KioskCredentialsModal } from './KioskCredentialsModal';

export function QrPanel({ canGenerate }: { canGenerate: boolean }) {
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
    const timer = setTimeout(handleRefresh, 500);
    return () => clearTimeout(timer);
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
            <p className="text-sm text-gray-500 px-2">{t('attendance.qrPanel.showHint')}</p>
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
          <div
            className={`relative rounded-2xl overflow-hidden p-3 transition-opacity ${
              isExpired ? 'opacity-30' : 'opacity-100'
            }`}
          >
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
              <span
                className={`font-semibold tabular-nums ${
                  timeLeft <= 10 ? 'text-red-500' : 'text-gray-700'
                }`}
              >
                {t('attendance.kiosk.secondsShort', { count: timeLeft })}
              </span>
            </div>
            <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${
                  timeLeft <= 10 ? 'bg-red-400' : 'bg-violet-500'
                }`}
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
