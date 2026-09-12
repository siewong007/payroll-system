import { useState } from 'react';
import { useNavigate } from 'react-router';
import { KeyRound } from 'lucide-react';
import api from '@/api/client';
import { useAuth } from '@/context/AuthContext';
import { PASSWORD_POLICY_HINT, validatePassword } from '@/lib/password';
import { getErrorMessage } from '@/lib/utils';

export function ChangePasswordCard() {
  const { logout } = useAuth();
  const navigate = useNavigate();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (newPassword !== confirmPassword) {
      setError('New passwords do not match');
      return;
    }
    const policyError = validatePassword(newPassword);
    if (policyError) {
      setError(policyError);
      return;
    }

    setLoading(true);
    try {
      await api.put('/auth/change-password', {
        current_password: currentPassword,
        new_password: newPassword,
      });
      // The backend revokes every session on password change — including this
      // one — so there is nothing to stay signed in with.
      await logout();
      navigate('/login');
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to change password'));
      setLoading(false);
    }
  };

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <KeyRound className="w-4 h-4 text-gray-400" />
          <span className="section-title">Password</span>
        </div>
      </div>

      <p className="text-sm text-gray-500 mb-4">
        Change the password you use to sign in. You will be signed out on all devices.
      </p>

      {error && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl mb-4">{error}</div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="form-label" htmlFor="cp-current">Current password</label>
          <input
            id="cp-current"
            type="password"
            autoComplete="current-password"
            className="form-input"
            value={currentPassword}
            onChange={(e) => setCurrentPassword(e.target.value)}
            required
          />
        </div>
        <div>
          <label className="form-label" htmlFor="cp-new">New password</label>
          <input
            id="cp-new"
            type="password"
            autoComplete="new-password"
            className="form-input"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            required
          />
          <p className="text-xs text-gray-400 mt-1">{PASSWORD_POLICY_HINT}</p>
        </div>
        <div>
          <label className="form-label" htmlFor="cp-confirm">Confirm new password</label>
          <input
            id="cp-confirm"
            type="password"
            autoComplete="new-password"
            className="form-input"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            required
          />
        </div>
        <div className="flex justify-end">
          <button type="submit" disabled={loading} className="btn-primary">
            {loading ? 'Changing…' : 'Change password'}
          </button>
        </div>
      </form>
    </section>
  );
}
