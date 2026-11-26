import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, ShieldCheck } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { TurnstileWidget } from '@/components/TurnstileWidget';
import { turnstileEnabled, useTurnstileWaiter } from '@/lib/useTurnstileWaiter';
import { getErrorMessage } from '@/lib/utils';
import type { User } from '@/types';

interface TwoFactorPromptProps {
  mfaToken: string;
  onSuccess: (user: User) => void;
  onBack?: () => void;
}

// Second step of login when the account has TOTP 2FA enabled — shared by
// the password-login flow (Login.tsx) and the Google OAuth callback.
export function TwoFactorPrompt({ mfaToken, onSuccess, onBack }: TwoFactorPromptProps) {
  const { t } = useTranslation();
  const [code, setCode] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const { completeTwoFactorLogin } = useAuth();
  const {
    widgetRef: turnstileRef,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
    resetWidget: resetTurnstileWidget,
  } = useTurnstileWaiter();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const captcha = await waitForTurnstileToken();
      if (turnstileEnabled() && !captcha) {
        setError(t('auth.verificationSlow'));
        return;
      }
      const user = captcha
        ? await completeTwoFactorLogin(mfaToken, code.trim(), captcha)
        : await completeTwoFactorLogin(mfaToken, code.trim());
      onSuccess(user);
    } catch (err: unknown) {
      resetTurnstileWidget();
      setError(getErrorMessage(err, t('auth.twoFactor.invalidOrExpired')));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="text-center">
        <div className="w-12 h-12 bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center mx-auto mb-3">
          <ShieldCheck className="w-6 h-6 text-gray-500 dark:text-slate-300" />
        </div>
        <p className="text-sm font-medium text-gray-900 dark:text-slate-100">{t('auth.twoFactor.title')}</p>
        <p className="text-sm text-gray-500 dark:text-slate-400 mt-1">
          {t('auth.twoFactor.prompt')}
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-5">
        {error && (
          <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
            {error}
          </div>
        )}

        <input
          type="text"
          inputMode="text"
          autoComplete="one-time-code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          aria-label={t('auth.twoFactor.title')}
          className="form-input !p-2.5 text-center tracking-widest"
          placeholder="123456"
          autoFocus
          required
        />

        <div className="flex justify-center">
          <TurnstileWidget
            ref={turnstileRef}
            action="totp_verify"
            onVerify={handleTurnstileVerify}
            onExpire={handleTurnstileExpire}
            onError={handleTurnstileError}
          />
        </div>

        <button
          type="submit"
          disabled={loading || !code.trim()}
          className="btn-primary w-full inline-flex items-center justify-center gap-2"
        >
          {loading ? (
            <>
              <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
              {t('auth.twoFactor.verifying')}
            </>
          ) : (
            t('auth.twoFactor.verify')
          )}
        </button>

        {onBack && (
          <button
            type="button"
            onClick={onBack}
            className="w-full min-h-11 inline-flex items-center justify-center rounded-xl text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
          >
            {t('auth.backToLogin')}
          </button>
        )}
      </form>
    </div>
  );
}
