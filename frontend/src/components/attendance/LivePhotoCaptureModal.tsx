import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Camera, RotateCcw, TriangleAlert } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';

const MAX_DATA_URL_BYTES = 500 * 1024;
const TARGET_MAX_WIDTH = 640;

interface LivePhotoCaptureModalProps {
  open: boolean;
  title: string;
  description?: string;
  required: boolean;
  submitting?: boolean;
  onConfirm: (photoBase64: string | null) => void;
  onCancel: () => void;
}

/**
 * Live-only camera capture for attendance check-in/out.
 * Uses navigator.mediaDevices.getUserMedia strictly (no file upload picker).
 */
export function LivePhotoCaptureModal({
  open,
  title,
  description,
  required,
  submitting = false,
  onConfirm,
  onCancel,
}: LivePhotoCaptureModalProps) {
  const { t } = useTranslation();
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const [cameraError, setCameraError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [captured, setCaptured] = useState<string | null>(null);

  const stopStream = useCallback(() => {
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
  }, []);

  const startCamera = useCallback(async () => {
    stopStream();
    setCameraError(null);
    setCaptured(null);
    if (!navigator.mediaDevices?.getUserMedia) {
      setCameraError(t('attendance.photo.cameraUnavailable'));
      return;
    }
    setStarting(true);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'user', width: { ideal: 720 }, height: { ideal: 720 } },
        audio: false,
      });
      streamRef.current = stream;
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        await videoRef.current.play().catch(() => undefined);
      }
    } catch {
      setCameraError(t('attendance.photo.cameraDenied'));
    } finally {
      setStarting(false);
    }
  }, [stopStream, t]);

  useEffect(() => {
    if (!open) {
      stopStream();
      setCaptured(null);
      setCameraError(null);
      return;
    }
    void startCamera();
    return () => stopStream();
  }, [open, startCamera, stopStream]);

  const handleCapture = () => {
    const video = videoRef.current;
    if (!video || !video.videoWidth || !video.videoHeight) return;
    const scale = Math.min(1, TARGET_MAX_WIDTH / video.videoWidth);
    const width = Math.max(1, Math.round(video.videoWidth * scale));
    const height = Math.max(1, Math.round(video.videoHeight * scale));
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.drawImage(video, 0, 0, width, height);

    let dataUrl = canvas.toDataURL('image/jpeg', 0.8);
    if (dataUrl.length > MAX_DATA_URL_BYTES) {
      dataUrl = canvas.toDataURL('image/jpeg', 0.6);
    }
    stopStream();
    setCaptured(dataUrl);
  };

  const handleRetake = () => {
    setCaptured(null);
    void startCamera();
  };

  const handleClose = () => {
    if (submitting) return;
    stopStream();
    onCancel();
  };

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title={title}
      maxWidth="max-w-md"
      footer={
        <div className="flex flex-wrap items-center justify-end gap-2">
          <button
            type="button"
            onClick={handleClose}
            disabled={submitting}
            className="btn-secondary"
          >
            {t('common.cancel')}
          </button>
          {!required && !captured && (
            <button
              type="button"
              onClick={() => onConfirm(null)}
              disabled={submitting}
              className="btn-secondary"
            >
              {t('attendance.photo.skipOptional')}
            </button>
          )}
          {captured && (
            <>
              <button
                type="button"
                onClick={handleRetake}
                disabled={submitting}
                className="btn-secondary flex items-center gap-1.5"
              >
                <RotateCcw className="w-4 h-4" />
                {t('attendance.photo.retake')}
              </button>
              <button
                type="button"
                onClick={() => onConfirm(captured)}
                disabled={submitting}
                className="btn-primary"
              >
                {submitting ? t('common.saving') : t('attendance.photo.confirmUse')}
              </button>
            </>
          )}
        </div>
      }
    >
      <div className="space-y-3">
        {description && (
          <p className="text-sm text-gray-600 dark:text-slate-400">{description}</p>
        )}

        <div className="relative overflow-hidden rounded-xl bg-slate-950 aspect-[4/3] flex items-center justify-center border border-gray-200 dark:border-slate-700">
          {captured ? (
            <img
              src={captured}
              alt={t('attendance.photo.previewAlt')}
              className="w-full h-full object-cover"
            />
          ) : cameraError ? (
            <div className="p-6 text-center text-sm text-amber-200 flex flex-col items-center gap-2">
              <TriangleAlert className="w-6 h-6 text-amber-400" />
              <p>{cameraError}</p>
              <button
                type="button"
                onClick={() => void startCamera()}
                className="mt-1 px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20 text-xs font-medium text-white"
              >
                {t('common.retry')}
              </button>
            </div>
          ) : (
            <video
              ref={videoRef}
              autoPlay
              playsInline
              muted
              className="w-full h-full object-cover scale-x-[-1]"
            />
          )}
        </div>

        {!captured && !cameraError && (
          <button
            type="button"
            onClick={handleCapture}
            disabled={starting || submitting}
            className="w-full btn-primary flex items-center justify-center gap-2 py-2.5"
          >
            <Camera className="w-4 h-4" />
            {starting ? t('common.loading') : t('attendance.photo.capture')}
          </button>
        )}
      </div>
    </Modal>
  );
}
