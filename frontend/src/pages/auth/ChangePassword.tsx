import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Trans, useTranslation } from 'react-i18next';
import { useAuth } from '@/context/AuthContext';
import api from '@/api/client';
import { getErrorMessage } from '@/lib/utils';
import { hasOnlyEmployeeRole } from '@/lib/roles';

export function ChangePassword() {
  const { user, logout } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSkip = async () => {
    try {
      await api.put('/auth/skip-change-password');
      if (user) {
        const updatedUser = { ...user, must_change_password: false };
        localStorage.setItem('user', JSON.stringify(updatedUser));
      }
      navigate(hasOnlyEmployeeRole(user) ? '/portal' : '/');
      window.location.reload();
    } catch {
      navigate(hasOnlyEmployeeRole(user) ? '/portal' : '/');
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (newPassword !== confirmPassword) {
      setError(t('auth.change.mismatch'));
      return;
    }

    if (newPassword.length < 10) {
      setError(t('auth.change.tooShort'));
      return;
    }

    setLoading(true);
    try {
      await api.put('/auth/change-password', {
        current_password: currentPassword,
        new_password: newPassword,
      });

      // Update local user state to clear the flag
      if (user) {
        const updatedUser = { ...user, must_change_password: false };
        localStorage.setItem('user', JSON.stringify(updatedUser));
      }

      // Logout so user logs in with new password
      await logout();
      navigate('/login');
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.change.failed')));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <div className="w-full max-w-md">
        <div className="bg-white rounded-2xl shadow-lg p-8">
          <div className="text-center mb-6">
            <div className="w-12 h-12 bg-amber-100 rounded-full flex items-center justify-center mx-auto mb-3">
              <svg className="w-6 h-6 text-amber-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
            </div>
            <h1 className="text-xl font-bold text-gray-900">{t('auth.change.title')}</h1>
            <p className="text-sm text-gray-500 mt-1">
              {t('auth.change.body')}
            </p>
            <p className="text-xs text-amber-600 bg-amber-50 rounded-lg px-3 py-2 mt-3">
              <Trans
                i18nKey="auth.change.defaultNote"
                values={{ code: 'Welcome@123' }}
                components={{ code: <span className="font-mono" /> }}
              />
            </p>
          </div>

          {error && (
            <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100 mb-4">
              {error}
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('auth.change.current')}</label>
              <input
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
                placeholder={t('auth.change.currentPlaceholder')}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('auth.change.new')}</label>
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
                minLength={10}
                placeholder={t('auth.change.newPlaceholder')}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('auth.change.confirm')}</label>
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
                placeholder={t('auth.change.confirmPlaceholder')}
              />
            </div>
            <button
              type="submit"
              disabled={loading}
              className="w-full bg-black text-white py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors"
            >
              {loading ? t('auth.change.submitting') : t('auth.change.submit')}
            </button>
          </form>
          <button
            onClick={handleSkip}
            className="w-full mt-3 text-sm text-gray-400 hover:text-gray-600 transition-colors"
          >
            {t('auth.change.skip')}
          </button>
        </div>
      </div>
    </div>
  );
}
