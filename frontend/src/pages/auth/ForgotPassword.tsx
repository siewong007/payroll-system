import { useRef, useState } from 'react';
import { Link } from 'react-router';
import { motion } from 'framer-motion';
import { useTranslation } from 'react-i18next';
import { forgotPassword } from '@/api/admin';
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';

export function ForgotPassword() {
  const { t } = useTranslation();
  const [email, setEmail] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const turnstileRef = useRef<TurnstileWidgetRef>(null);
  const turnstileToken = useRef<string | undefined>(undefined);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    const token = turnstileToken.current;
    turnstileToken.current = undefined;
    try {
      await forgotPassword(email, token);
      setSubmitted(true);
    } catch {
      turnstileRef.current?.reset();
      setError(t('auth.forgot.failed'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100">
      <motion.div
        className="w-full max-w-md px-4"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
      >
        <div className="bg-white rounded-2xl shadow p-6 sm:p-8">
          <div className="text-center mb-8">
            <div className="w-12 h-12 bg-black rounded-xl flex items-center justify-center mx-auto mb-4">
              <span className="text-white font-bold text-lg">P</span>
            </div>
            <h1 className="text-xl font-semibold text-gray-900">{t('auth.forgot.title')}</h1>
            <p className="text-sm text-gray-400 mt-1">
              {t('auth.forgot.subtitle')}
            </p>
          </div>

          {submitted ? (
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-green-50 rounded-full flex items-center justify-center mx-auto">
                <svg className="w-8 h-8 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <div>
                <p className="text-sm text-gray-600">
                  {t('auth.forgot.sentBody')}
                </p>
              </div>
              <Link
                to="/login"
                className="inline-block text-sm text-black font-medium hover:underline"
              >
                {t('auth.backToLogin')}
              </Link>
            </div>
          ) : (
            <form onSubmit={handleSubmit} className="space-y-5">
              {error && (
                <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl">
                  {error}
                </div>
              )}

              <div>
                <label className="form-label">{t('auth.email')}</label>
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="border p-2.5 rounded-lg w-full text-sm outline-none focus:border-black transition-colors"
                  placeholder={t('auth.emailPlaceholder')}
                  required
                />
              </div>

              <TurnstileWidget
                ref={turnstileRef}
                onVerify={(t) => {
                  turnstileToken.current = t;
                }}
                onExpire={() => {
                  turnstileToken.current = undefined;
                }}
                onError={() => {
                  turnstileToken.current = undefined;
                }}
              />

              <button
                type="submit"
                disabled={loading}
                className="w-full bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 disabled:opacity-50 transition-all"
              >
                {loading ? t('auth.submitting') : t('auth.forgot.submit')}
              </button>

              <div className="text-center">
                <Link to="/login" className="text-sm text-gray-500 hover:text-gray-700">
                  {t('auth.backToLogin')}
                </Link>
              </div>
            </form>
          )}
        </div>
      </motion.div>
    </div>
  );
}
