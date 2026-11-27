import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import QRCode from 'qrcode';
import { AlertCircle, QrCode, RefreshCw } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import {
  createPersonalQr, issueDeviceChallenge, type DeviceBindingMode, type PersonalQrResponse,
} from '@/api/attendance';
import { getEnrolledDeviceId, signDeviceChallenge } from '@/lib/deviceKey';
import { getErrorMessage } from '@/lib/utils';

interface PersonalKioskPassModalProps {
  open: boolean;
  onClose: () => void;
  hasOpenSession: boolean;
  deviceMode: DeviceBindingMode;
  scopeKey: string;
}

export function PersonalKioskPassModal({
  open,
  onClose,
  hasOpenSession,
  deviceMode,
  scopeKey,
}: PersonalKioskPassModalProps) {
  const { t } = useTranslation();
  const [pass, setPass] = useState<PersonalQrResponse | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [secondsLeft, setSecondsLeft] = useState(0);

  const action: 'check_in' | 'check_out' = hasOpenSession ? 'check_out' : 'check_in';

  const generatePass = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      let deviceProof;
      const enrolledId = await getEnrolledDeviceId(scopeKey);
      if (enrolledId) {
        try {
          const challenge = await issueDeviceChallenge('personal_qr');
          const proof = await signDeviceChallenge(challenge, 'personal_qr', scopeKey, enrolledId);
          if (proof) deviceProof = proof;
        } catch {
          if (deviceMode === 'enforce') {
            throw new Error(t('attendance.device.signFailed'));
          }
        }
      }
      const next = await createPersonalQr({ action, device_proof: deviceProof });
      const dataUrl = await QRCode.toDataURL(next.qr_payload, {
        width: 260,
        margin: 2,
        color: { dark: '#0f172a', light: '#ffffff' },
      });
      setPass(next);
      setQrDataUrl(dataUrl);
      const remain = Math.max(0, Math.round((new Date(next.expires_at).getTime() - Date.now()) / 1000));
      setSecondsLeft(remain || next.ttl_seconds);
    } catch (err) {
      setError(getErrorMessage(err, t('attendance.kioskPass.failed')));
    } finally {
      setLoading(false);
    }
  }, [action, deviceMode, scopeKey, t]);

  useEffect(() => {
    if (!open) {
      setPass(null);
      setQrDataUrl('');
      setError(null);
      setSecondsLeft(0);
      return;
    }
    void generatePass();
  }, [open, generatePass]);

  useEffect(() => {
    if (!open || !pass) return;
    const id = window.setInterval(() => {
      const remain = Math.max(0, Math.round((new Date(pass.expires_at).getTime() - Date.now()) / 1000));
      setSecondsLeft(remain);
      if (remain <= 0) {
        void generatePass();
      }
    }, 1000);
    return () => window.clearInterval(id);
  }, [open, pass, generatePass]);

  const ttl = pass?.ttl_seconds || 60;
  const pct = Math.min(100, Math.max(0, (secondsLeft / ttl) * 100));

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('attendance.kioskPass.title')}
      maxWidth="max-w-sm"
      footer={
        <div className="flex items-center justify-end gap-2">
          <button type="button" onClick={() => void generatePass()} disabled={loading} className="btn-secondary flex items-center gap-1.5">
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            {t('common.refresh')}
          </button>
          <button type="button" onClick={onClose} className="btn-primary">
            {t('common.close')}
          </button>
        </div>
      }
    >
      <div className="flex flex-col items-center text-center space-y-4">
        <span className="inline-flex items-center gap-1.5 rounded-full bg-[var(--accent-soft)] text-[var(--accent-1)] px-3 py-1 text-xs font-semibold">
          <QrCode className="w-3.5 h-3.5" />
          {action === 'check_out' ? t('attendance.kioskPass.checkOutBadge') : t('attendance.kioskPass.checkInBadge')}
        </span>

        {error ? (
          <div className="w-full rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-4 text-sm text-red-700 dark:text-red-300 flex items-start gap-2 text-left">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <span>{error}</span>
          </div>
        ) : qrDataUrl ? (
          <div className="p-3 rounded-xl bg-white border border-[var(--border)]">
            <img src={qrDataUrl} alt={t('attendance.kioskPass.title')} className="w-56 h-56" />
          </div>
        ) : (
          <div className="w-56 h-56 rounded-xl bg-[var(--surface-subtle)] animate-pulse" />
        )}

        {pass && !error && (
          <div className="w-full space-y-1.5">
            <div className="w-full h-1.5 rounded-full bg-[var(--surface-subtle)] overflow-hidden">
              <div
                className="h-full bg-[var(--accent-1)] transition-all duration-1000"
                style={{ width: `${pct}%` }}
              />
            </div>
            <p className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
              {t('attendance.kioskPass.expiresIn', { seconds: secondsLeft })}
            </p>
          </div>
        )}

        <p className="text-xs text-gray-500 dark:text-slate-400">
          {t('attendance.kioskPass.instruction')}
        </p>
      </div>
    </Modal>
  );
}
