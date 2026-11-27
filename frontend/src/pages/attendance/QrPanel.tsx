import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertCircle, Eye, EyeOff, Link2, QrCode, RefreshCw } from 'lucide-react';
import QRCode from 'qrcode';
import { generateQrToken } from '@/api/attendance';
import { KioskCredentialsModal } from './KioskCredentialsModal';

export function QrPanel({ canGenerate, canManageKiosks = true }: { canGenerate: boolean; canManageKiosks?: boolean }) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [timeLeft, setTimeLeft] = useState(0);
  const [isExpired, setIsExpired] = useState(false);
  const [showKiosks, setShowKiosks] = useState(false);
  // Opt-in display. The panel used to mint a token every 5 minutes for as long
  // as any admin tab sat open — even one only viewing records.
  const [showQr, setShowQr] = useState(false);
  const queryClient = useQueryClient();

  const { data: token, dataUpdatedAt, refetch: generateNew, isLoading, isFetching, isError } = useQuery({
    queryKey: ['attendance-qr'],
    queryFn: generateQrToken,
    enabled: showQr && canGenerate,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });

  // Draw QR on canvas, clearing qrcode's fixed inline px dimensions so CSS width
  // constraints hold even in narrow containers. `showQr` is a dependency because
  // the canvas is unmounted while hidden: re-showing within the TTL returns the
  // same cached token object, so keying on `token` alone left the fresh canvas
  // unpainted until the token expired.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!showQr || !token || !canvas) return;
    let cancelled = false;
    const clearInlineSize = () => {
      canvas.style.removeProperty('width');
      canvas.style.removeProperty('height');
    };
    void Promise.resolve(
      QRCode.toCanvas(canvas, token.scan_url, {
        width: 240,
        margin: 2,
        errorCorrectionLevel: 'M',
        color: { dark: '#111827', light: '#ffffff' },
      }),
    )
      .then(() => {
        if (!cancelled) clearInlineSize();
      })
      .catch(() => {});
    clearInlineSize();
    return () => {
      cancelled = true;
    };
  }, [token, showQr]);

  // Countdown timer, measured from the TTL against the moment the response
  // arrived here. `expires_at` is a *server* instant, so comparing it with
  // Date.now() folded the device's clock offset into the countdown.
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
    <div className="card p-5 sm:p-6 flex flex-col items-center gap-4">
      <div className="flex items-center justify-between w-full gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <QrCode className="w-5 h-5 text-violet-600 dark:text-violet-400 shrink-0" aria-hidden="true" />
          <h2 className="font-semibold text-gray-900 dark:text-slate-100 truncate">{t('attendance.qrPanel.title')}</h2>
        </div>
        {showQr && canGenerate && (
          <button
            type="button"
            onClick={handleRefresh}
            disabled={isFetching}
            className="min-h-11 sm:min-h-9 flex items-center gap-1.5 text-xs font-medium text-gray-700 dark:text-slate-200 hover:text-gray-900 dark:hover:text-white bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 px-3 py-1.5 rounded-lg transition-colors disabled:opacity-50 shrink-0"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isFetching ? 'animate-spin' : ''}`} aria-hidden="true" />
            {t('common.refresh')}
          </button>
        )}
      </div>

      {!canGenerate ? (
        <p className="text-sm text-gray-500 dark:text-slate-400 text-center py-4">
          {t('attendance.qrPanel.noPermission')}
        </p>
      ) : !showQr ? (
        <>
          <div className="w-full py-6 flex flex-col items-center gap-2 text-center">
            <div className="w-14 h-14 rounded-2xl bg-violet-50 dark:bg-violet-500/15 flex items-center justify-center">
              <QrCode className="w-7 h-7 text-violet-500 dark:text-violet-400" aria-hidden="true" />
            </div>
            <p className="text-sm text-gray-500 dark:text-slate-400 px-2">{t('attendance.qrPanel.showHint')}</p>
          </div>
          <button
            type="button"
            onClick={() => setShowQr(true)}
            className="btn-primary w-full rounded-xl"
          >
            <Eye className="w-4 h-4" aria-hidden="true" />
            {t('attendance.qrPanel.showButton')}
          </button>
        </>
      ) : isError ? (
        <div role="alert" className="w-full py-6 flex flex-col items-center gap-3 text-center">
          <AlertCircle className="w-8 h-8 text-red-500 dark:text-red-400" aria-hidden="true" />
          <p className="text-sm text-gray-600 dark:text-slate-300">{t('attendance.qrPanel.generateFailed')}</p>
          <button
            type="button"
            onClick={handleRefresh}
            disabled={isFetching}
            className="btn-secondary min-h-11 sm:min-h-9 text-xs"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isFetching ? 'animate-spin' : ''}`} aria-hidden="true" /> {t('common.retry')}
          </button>
        </div>
      ) : isLoading || !token ? (
        <div
          role="status"
          aria-label={t('common.loading')}
          className="w-full max-w-[240px] aspect-square rounded-2xl bg-gray-100 dark:bg-slate-800 motion-safe:animate-pulse flex items-center justify-center"
        >
          <div className="spinner" />
        </div>
      ) : (
        <>
          {/* QR Canvas */}
          <div className="relative w-full max-w-[264px] rounded-2xl bg-white border border-gray-200/80 dark:border-transparent p-3 shadow-xs">
            <canvas
              ref={canvasRef}
              role="img"
              aria-label={t('attendance.qrPanel.title')}
              className={`block !w-full !h-auto max-w-[240px] aspect-square mx-auto rounded-xl transition-opacity ${
                isExpired ? 'opacity-30' : 'opacity-100'
              }`}
            />
            {isExpired && (
              <div className="absolute inset-0 flex items-center justify-center">
                <div
                  role="status"
                  className="bg-white/95 dark:bg-slate-900/95 border border-gray-200 dark:border-slate-700 rounded-xl px-4 py-2 text-sm font-semibold text-gray-700 dark:text-slate-200 shadow-sm"
                >
                  {t('attendance.qrPanel.refreshing')}
                </div>
              </div>
            )}
          </div>

          {/* Countdown */}
          <div className="w-full">
            <div className="flex justify-between text-xs text-gray-500 dark:text-slate-400 mb-1">
              <span>{t('attendance.qrPanel.expiresIn')}</span>
              <span
                className={`font-semibold tabular-nums ${
                  timeLeft <= 10 ? 'text-red-600 dark:text-red-400' : 'text-gray-700 dark:text-slate-200'
                }`}
              >
                {t('attendance.kiosk.secondsShort', { count: timeLeft })}
              </span>
            </div>
            <div
              role="progressbar"
              aria-label={t('attendance.qrPanel.expiresIn')}
              aria-valuemin={0}
              aria-valuemax={token.ttl_seconds || 300}
              aria-valuenow={timeLeft}
              className="h-1.5 bg-gray-100 dark:bg-slate-800 rounded-full overflow-hidden"
            >
              <div
                className={`h-full rounded-full transition-all ${
                  timeLeft <= 10 ? 'bg-red-500' : 'bg-violet-500'
                }`}
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>

          <p className="text-xs text-gray-500 dark:text-slate-400 text-center">
            {t('attendance.qrPanel.rotateNote', { minutes: ttlMinutes })}
          </p>

          <button
            type="button"
            onClick={() => setShowQr(false)}
            className="min-h-11 sm:min-h-9 px-3 rounded-lg text-xs font-medium text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 flex items-center gap-1.5 transition-colors"
          >
            <EyeOff className="w-3.5 h-3.5" aria-hidden="true" /> {t('attendance.qrPanel.hide')}
          </button>
        </>
      )}

      {canManageKiosks && (
        <button
          type="button"
          onClick={() => setShowKiosks(true)}
          className="btn-secondary w-full rounded-xl"
        >
          <Link2 className="w-4 h-4" aria-hidden="true" />
          {t('attendance.kioskModal.title')}
        </button>
      )}

      {showKiosks && <KioskCredentialsModal onClose={() => setShowKiosks(false)} />}
    </div>
  );
}
