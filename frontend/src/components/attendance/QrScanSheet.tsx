import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Html5Qrcode as Html5QrcodeInstance } from 'html5-qrcode';
import { AlertCircle, Camera, Flashlight, Keyboard, Loader2, RefreshCw, X } from 'lucide-react';
import { parseScanToken } from '@/lib/attendance';
import { getErrorMessage } from '@/lib/utils';

interface QrScanSheetProps {
  onClose: () => void;
  /** Fired once per scan with the extracted token. The parent submits it. */
  onToken: (token: string) => void;
  /** Clears any active parent submit error when user explicitly retries the scan. */
  onRetryScan?: () => void;
  /** Fired when the camera is live — the parent uses it to pre-warm the GPS fix. */
  onCameraReady?: () => void;
  /** True while the parent's check-in request is in flight. */
  busy?: boolean;
  /** Server-side failure from the parent's submit, shown in place. */
  errorText?: string;
}

/**
 * Full-screen QR capture.
 *
 * Replaces the old 256 px-tall preview inside a dialog. The token is handed
 * straight back to the caller, which submits the check-in without leaving the
 * page — the previous flow navigated to `/attendance/scan`, dropping the
 * employee out of the portal shell and re-fetching the attendance method that
 * had been in the React Query cache seconds earlier.
 */
export function QrScanSheet({ onClose, onToken, onRetryScan, onCameraReady, busy, errorText }: QrScanSheetProps) {
  const { t } = useTranslation();
  // html5-qrcode addresses its mount point by id, so give each instance its own.
  const readerId = `qr-reader-${useId().replace(/:/g, '')}`;

  const [phase, setPhase] = useState<'starting' | 'scanning' | 'failed'>('starting');
  const [cameraAttempt, setCameraAttempt] = useState(0);
  const [cameraError, setCameraError] = useState('');
  const [scanError, setScanError] = useState('');
  const [torchOn, setTorchOn] = useState(false);
  const [torchAvailable, setTorchAvailable] = useState(false);
  const [manualOpen, setManualOpen] = useState(false);
  const [manualCode, setManualCode] = useState('');

  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const scannerRef = useRef<Html5QrcodeInstance | null>(null);
  // The success callback fires repeatedly while the code stays in frame.
  const claimedRef = useRef(false);
  const lastSubmittedTokenRef = useRef<string | null>(null);
  const lastRejectedTokenRef = useRef<string | null>(null);
  // Hold the latest callbacks so the camera effect can stay mounted across the
  // parent's re-renders. Synced in an effect rather than during render — the
  // React Compiler lint rejects a ref write in the render pass.
  const onTokenRef = useRef(onToken);
  const onCameraReadyRef = useRef(onCameraReady);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onTokenRef.current = onToken;
    onCameraReadyRef.current = onCameraReady;
    onCloseRef.current = onClose;
  });

  // Escape close, focus restoration, focus trap & body scroll lock
  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    closeButtonRef.current?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCloseRef.current();
        return;
      }
      if (e.key !== 'Tab' || !dialogRef.current) return;
      const focusables = dialogRef.current.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.body.style.overflow = prevOverflow;
      previousFocus?.focus();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const handleDecoded = (decodedText: string) => {
      if (claimedRef.current) return;
      const parsed = parseScanToken(decodedText);
      if ('error' in parsed) {
        // Keep the camera running — the employee just needs a different code.
        setScanError(parsed.error);
        return;
      }
      if (parsed.token === lastRejectedTokenRef.current) {
        return;
      }
      claimedRef.current = true;
      lastSubmittedTokenRef.current = parsed.token;
      setScanError('');
      onTokenRef.current(parsed.token);
    };

    const start = async () => {
      try {
        const { Html5Qrcode, Html5QrcodeSupportedFormats } = await import('html5-qrcode');
        if (cancelled) return;
        const scanner = new Html5Qrcode(readerId, {
          verbose: false,
          // Decoding only QR is markedly faster than sweeping every barcode family,
          // and the native detector (where present) beats the JS decoder outright.
          formatsToSupport: [Html5QrcodeSupportedFormats.QR_CODE],
          useBarCodeDetectorIfSupported: true,
        });
        scannerRef.current = scanner;

        await scanner.start(
          { facingMode: 'environment' },
          {
            fps: 10,
            // Track the viewport instead of a fixed 280 px box, which overflowed
            // small screens and wasted the frame on large ones.
            qrbox: (w, h) => {
              const edge = Math.max(160, Math.round(Math.min(w, h) * 0.72));
              return { width: edge, height: edge };
            },
            aspectRatio: 1,
          },
          handleDecoded,
          () => {
            /* fires once per undecoded frame — not an error */
          }
        );
        if (cancelled) {
          await scanner.stop().catch(() => {});
          try {
            scanner.clear();
          } catch {
            /* already cleared */
          }
          return;
        }
        setPhase('scanning');
        onCameraReadyRef.current?.();

        try {
          const caps: MediaTrackCapabilities = scanner.getRunningTrackCapabilities();
          if (caps && 'torch' in caps) setTorchAvailable(true);
        } catch {
          /* capability probing is best-effort */
        }
      } catch (err: unknown) {
        if (cancelled) return;
        setPhase('failed');
        setCameraError(
          getErrorMessage(err, t('attendance.qr.cameraFailed'))
        );
      }
    };

    void start();

    return () => {
      cancelled = true;
      const active = scannerRef.current;
      scannerRef.current = null;
      if (!active) return;
      if (active.isScanning) {
        active.stop().then(() => active.clear()).catch(() => { /* already torn down */ });
      } else {
        try {
          active.clear();
        } catch {
          /* nothing rendered yet */
        }
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- t is stable; adding it would restart the camera
  }, [readerId, cameraAttempt]);

  // Prevent 10fps infinite resubmission of the same rejected token while still
  // allowing a different QR token immediately or the same one after "Scan again".
  useEffect(() => {
    if (errorText) {
      lastRejectedTokenRef.current = lastSubmittedTokenRef.current;
      claimedRef.current = false;
    } else {
      lastRejectedTokenRef.current = null;
    }
  }, [errorText]);

  const toggleTorch = useCallback(async () => {
    const scanner = scannerRef.current;
    if (!scanner) return;
    const next = !torchOn;
    try {
      await scanner.applyVideoConstraints({
        advanced: [{ torch: next }],
      } as unknown as MediaTrackConstraints);
      setTorchOn(next);
    } catch {
      setTorchAvailable(false);
    }
  }, [torchOn]);

  const submitManual = () => {
    if (busy || claimedRef.current || !manualCode.trim()) return;
    const parsed = parseScanToken(manualCode);
    if ('error' in parsed) {
      setScanError(parsed.error);
      return;
    }
    lastRejectedTokenRef.current = null;
    lastSubmittedTokenRef.current = parsed.token;
    claimedRef.current = true;
    setScanError('');
    onTokenRef.current(parsed.token);
  };

  const handleRetryCamera = () => {
    setCameraError('');
    setTorchAvailable(false);
    setTorchOn(false);
    setPhase('starting');
    setManualOpen(false);
    setCameraAttempt((n) => n + 1);
  };

  const handleScanAgain = () => {
    lastRejectedTokenRef.current = null;
    claimedRef.current = false;
    setScanError('');
    onRetryScan?.();
  };

  const message = errorText || scanError;

  return (
    <div
      ref={dialogRef}
      className="fixed inset-0 z-[60] bg-black text-white flex flex-col"
      role="dialog"
      aria-modal="true"
      aria-label={t('attendance.qr.ariaLabel')}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-3 shrink-0">
        <div>
          <p className="text-sm font-semibold">{t('attendance.qr.title')}</p>
          <p className="text-xs text-white/70">{t('attendance.qr.subtitle')}</p>
        </div>
        <button
          ref={closeButtonRef}
          type="button"
          onClick={onClose}
          aria-label={t('attendance.qr.close')}
          className="w-11 h-11 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Camera */}
      <div className="relative flex-1 min-h-0 overflow-hidden">
        <div id={readerId} className="w-full h-full [&_video]:w-full [&_video]:h-full [&_video]:object-cover" />

        {phase === 'starting' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black">
            <Loader2 className="w-8 h-8 animate-spin text-white/70" />
            <p className="text-sm text-white/70">{t('attendance.qr.starting')}</p>
          </div>
        )}

        {phase === 'failed' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-black px-8 text-center">
            <AlertCircle className="w-10 h-10 text-amber-400" />
            <p className="text-sm text-white/80">{cameraError}</p>
            <div className="flex flex-wrap items-center justify-center gap-2">
              <button
                type="button"
                onClick={handleRetryCamera}
                className="inline-flex items-center gap-2 px-4 py-2.5 min-h-11 rounded-xl bg-white/15 text-white text-sm font-semibold hover:bg-white/25"
              >
                <RefreshCw className="w-4 h-4" />
                {t('attendance.qr.tryCameraAgain')}
              </button>
              <button
                type="button"
                onClick={() => setManualOpen(true)}
                className="px-4 py-2.5 min-h-11 rounded-xl bg-white text-gray-900 text-sm font-semibold"
              >
                {t('attendance.qr.enterCode')}
              </button>
            </div>
          </div>
        )}

        {/* Framing guides */}
        {phase === 'scanning' && !busy && (
          <div aria-hidden className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <div className="relative w-[72vmin] max-w-[20rem] aspect-square">
              {['top-0 left-0 border-t-4 border-l-4 rounded-tl-xl',
                'top-0 right-0 border-t-4 border-r-4 rounded-tr-xl',
                'bottom-0 left-0 border-b-4 border-l-4 rounded-bl-xl',
                'bottom-0 right-0 border-b-4 border-r-4 rounded-br-xl',
              ].map((corner) => (
                <span key={corner} className={`absolute w-9 h-9 border-emerald-400 ${corner}`} />
              ))}
            </div>
          </div>
        )}

        {/* Submitting */}
        {busy && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/85">
            <Loader2 className="w-8 h-8 animate-spin text-emerald-400" />
            <p className="text-sm font-medium">{t('attendance.qr.checkingIn')}</p>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="shrink-0 px-4 pt-3 pb-[max(1rem,env(safe-area-inset-bottom))] space-y-3">
        {message && (
          <div role="alert" className="flex items-start justify-between gap-2 rounded-xl bg-red-500/20 border border-red-500/40 px-4 py-3 text-sm text-red-100">
            <div className="flex items-start gap-2 min-w-0">
              <AlertCircle className="w-4 h-4 mt-0.5 shrink-0" />
              <span>{message}</span>
            </div>
            {errorText && (
              <button
                type="button"
                onClick={handleScanAgain}
                className="shrink-0 text-xs font-semibold underline underline-offset-2 text-white hover:text-emerald-300"
              >
                {t('attendance.qr.scanAgain')}
              </button>
            )}
          </div>
        )}

        {manualOpen ? (
          <div className="space-y-2">
            <div className="flex gap-2">
              <input
                value={manualCode}
                onChange={(e) => setManualCode(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') submitManual(); }}
                placeholder={t('attendance.qr.codePlaceholder')}
                aria-label={t('attendance.qr.codeAria')}
                autoComplete="off"
                autoCapitalize="none"
                spellCheck={false}
                className="flex-1 min-w-0 rounded-xl bg-white/10 border border-white/20 px-3 py-2.5 text-base sm:text-sm text-white placeholder:text-white/50 focus:outline-none focus:border-emerald-400"
              />
              <button
                type="button"
                onClick={submitManual}
                disabled={!manualCode.trim() || busy}
                className="px-4 min-h-11 rounded-xl bg-white text-gray-900 text-sm font-semibold disabled:opacity-40"
              >
                {t('attendance.qr.go')}
              </button>
            </div>
            <div className="flex justify-center">
              <button
                type="button"
                onClick={() => setManualOpen(false)}
                className="inline-flex items-center gap-1.5 text-xs text-white/75 hover:text-white py-1"
              >
                <Camera className="w-3.5 h-3.5" />
                {phase === 'failed' ? t('common.cancel') : t('attendance.qr.backToCamera')}
              </button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center gap-2">
            {torchAvailable && (
              <button
                type="button"
                onClick={() => void toggleTorch()}
                aria-pressed={torchOn}
                className={`flex items-center gap-2 px-4 py-2.5 min-h-11 rounded-xl text-sm font-medium transition-colors ${
                  torchOn ? 'bg-amber-400 text-gray-900' : 'bg-white/10 text-white hover:bg-white/20'
                }`}
              >
                <Flashlight className="w-4 h-4" />
                {t('attendance.qr.torch')}
              </button>
            )}
            <button
              type="button"
              onClick={() => setManualOpen(true)}
              className="flex items-center gap-2 px-4 py-2.5 min-h-11 rounded-xl bg-white/10 text-white text-sm font-medium hover:bg-white/20 transition-colors"
            >
              <Keyboard className="w-4 h-4" />
              {t('attendance.qr.enterCodeShort')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
