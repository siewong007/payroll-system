import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Html5Qrcode, Html5QrcodeSupportedFormats } from 'html5-qrcode';
import { AlertCircle, Flashlight, Keyboard, Loader2, X } from 'lucide-react';
import { parseScanToken } from '@/lib/attendance';
import { getErrorMessage } from '@/lib/utils';

interface QrScanSheetProps {
  onClose: () => void;
  /** Fired once per scan with the extracted token. The parent submits it. */
  onToken: (token: string) => void;
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
export function QrScanSheet({ onClose, onToken, onCameraReady, busy, errorText }: QrScanSheetProps) {
  // html5-qrcode addresses its mount point by id, so give each instance its own.
  const readerId = `qr-reader-${useId().replace(/:/g, '')}`;

  const [phase, setPhase] = useState<'starting' | 'scanning' | 'failed'>('starting');
  const [cameraError, setCameraError] = useState('');
  const [scanError, setScanError] = useState('');
  const [torchOn, setTorchOn] = useState(false);
  const [torchAvailable, setTorchAvailable] = useState(false);
  const [manualOpen, setManualOpen] = useState(false);
  const [manualCode, setManualCode] = useState('');

  const scannerRef = useRef<Html5Qrcode | null>(null);
  // The success callback fires repeatedly while the code stays in frame.
  const claimedRef = useRef(false);
  // Hold the latest callbacks so the camera effect can stay mounted across the
  // parent's re-renders. Synced in an effect rather than during render — the
  // React Compiler lint rejects a ref write in the render pass.
  const onTokenRef = useRef(onToken);
  const onCameraReadyRef = useRef(onCameraReady);
  useEffect(() => {
    onTokenRef.current = onToken;
    onCameraReadyRef.current = onCameraReady;
  });

  useEffect(() => {
    let cancelled = false;
    const scanner = new Html5Qrcode(readerId, {
      verbose: false,
      // Decoding only QR is markedly faster than sweeping every barcode family,
      // and the native detector (where present) beats the JS decoder outright.
      formatsToSupport: [Html5QrcodeSupportedFormats.QR_CODE],
      useBarCodeDetectorIfSupported: true,
    });
    scannerRef.current = scanner;

    const handleDecoded = (decodedText: string) => {
      if (claimedRef.current) return;
      const parsed = parseScanToken(decodedText);
      if ('error' in parsed) {
        // Keep the camera running — the employee just needs a different code.
        setScanError(parsed.error);
        return;
      }
      claimedRef.current = true;
      setScanError('');
      onTokenRef.current(parsed.token);
    };

    const start = async () => {
      try {
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
        if (cancelled) return;
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
          getErrorMessage(err, 'Could not start the camera. Check camera permission for this site in your browser settings.')
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
  }, [readerId]);

  // A rejected token must be scannable again.
  useEffect(() => {
    if (errorText) claimedRef.current = false;
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
    const parsed = parseScanToken(manualCode);
    if ('error' in parsed) {
      setScanError(parsed.error);
      return;
    }
    claimedRef.current = true;
    setScanError('');
    onTokenRef.current(parsed.token);
  };

  const message = errorText || scanError;

  return (
    <div className="fixed inset-0 z-[60] bg-black text-white flex flex-col" role="dialog" aria-modal="true" aria-label="Scan attendance QR code">
      {/* Header */}
      <div className="flex items-center justify-between px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-3 shrink-0">
        <div>
          <p className="text-sm font-semibold">Scan to check in</p>
          <p className="text-xs text-white/60">Point at the code on the kiosk screen</p>
        </div>
        <button
          onClick={onClose}
          aria-label="Close scanner"
          className="w-10 h-10 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
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
            <p className="text-sm text-white/70">Starting camera…</p>
          </div>
        )}

        {phase === 'failed' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-black px-8 text-center">
            <AlertCircle className="w-10 h-10 text-amber-400" />
            <p className="text-sm text-white/80">{cameraError}</p>
            <button
              onClick={() => setManualOpen(true)}
              className="px-4 py-2.5 rounded-xl bg-white text-gray-900 text-sm font-semibold"
            >
              Enter the code instead
            </button>
          </div>
        )}

        {/* Framing guides */}
        {phase === 'scanning' && !busy && (
          <div aria-hidden className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <div className="relative w-[72vmin] max-w-[20rem] aspect-square">
              {['top-0 left-0 border-t-4 border-l-4 rounded-tl-3xl',
                'top-0 right-0 border-t-4 border-r-4 rounded-tr-3xl',
                'bottom-0 left-0 border-b-4 border-l-4 rounded-bl-3xl',
                'bottom-0 right-0 border-b-4 border-r-4 rounded-br-3xl',
              ].map((corner) => (
                <span key={corner} className={`absolute w-10 h-10 border-emerald-400 ${corner}`} />
              ))}
            </div>
          </div>
        )}

        {/* Submitting */}
        {busy && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/80 backdrop-blur-sm">
            <Loader2 className="w-9 h-9 animate-spin text-emerald-400" />
            <p className="text-sm font-medium">Checking you in…</p>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="shrink-0 px-4 pt-3 pb-[max(1rem,env(safe-area-inset-bottom))] space-y-3">
        {message && (
          <div className="flex items-start gap-2 rounded-2xl bg-red-500/15 border border-red-500/30 px-4 py-3 text-sm text-red-100">
            <AlertCircle className="w-4 h-4 mt-0.5 shrink-0" />
            <span>{message}</span>
          </div>
        )}

        {manualOpen ? (
          <div className="flex gap-2">
            <input
              value={manualCode}
              onChange={(e) => setManualCode(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') submitManual(); }}
              placeholder="Paste or type the code"
              aria-label="Attendance code"
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              className="flex-1 min-w-0 rounded-xl bg-white/10 border border-white/20 px-3 py-2.5 text-sm placeholder:text-white/40 focus:outline-none focus:border-emerald-400"
            />
            <button
              onClick={submitManual}
              disabled={!manualCode.trim() || busy}
              className="px-4 rounded-xl bg-white text-gray-900 text-sm font-semibold disabled:opacity-40"
            >
              Go
            </button>
          </div>
        ) : (
          <div className="flex items-center justify-center gap-2">
            {torchAvailable && (
              <button
                onClick={() => void toggleTorch()}
                aria-pressed={torchOn}
                className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-colors ${
                  torchOn ? 'bg-amber-400 text-gray-900' : 'bg-white/10 text-white hover:bg-white/20'
                }`}
              >
                <Flashlight className="w-4 h-4" />
                Torch
              </button>
            )}
            <button
              onClick={() => setManualOpen(true)}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-white/10 text-white text-sm font-medium hover:bg-white/20 transition-colors"
            >
              <Keyboard className="w-4 h-4" />
              Enter code
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
