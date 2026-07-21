import { useState } from 'react';
import { ShieldCheck } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
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
  const [code, setCode] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const { completeTwoFactorLogin } = useAuth();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const user = await completeTwoFactorLogin(mfaToken, code.trim());
      onSuccess(user);
    } catch (err: unknown) {
      setError(getErrorMessage(err, 'Invalid or expired code'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="text-center">
        <div className="w-12 h-12 bg-gray-100 rounded-full flex items-center justify-center mx-auto mb-3">
          <ShieldCheck className="w-6 h-6 text-gray-500" />
        </div>
        <p className="text-sm font-medium text-gray-900">Two-factor authentication</p>
        <p className="text-sm text-gray-500 mt-1">
          Enter the 6-digit code from your authenticator app, or a backup code.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-5">
        {error && (
          <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl">{error}</div>
        )}

        <input
          type="text"
          inputMode="numeric"
          autoComplete="one-time-code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          className="border p-2.5 rounded-lg w-full text-sm text-center tracking-widest outline-none focus:border-black transition-colors"
          placeholder="123456"
          autoFocus
          required
        />

        <button
          type="submit"
          disabled={loading || !code.trim()}
          className="w-full bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 disabled:opacity-50 transition-all"
        >
          {loading ? 'Verifying...' : 'Verify'}
        </button>

        {onBack && (
          <button
            type="button"
            onClick={onBack}
            className="w-full text-center text-sm text-gray-500 hover:text-gray-700"
          >
            Back to login
          </button>
        )}
      </form>
    </div>
  );
}
