import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '@/context/AuthContext';

export function OAuth2Callback() {
  const navigate = useNavigate();
  const { setSession } = useAuth();
  const [error, setError] = useState('');

  useEffect(() => {
    const hash = window.location.hash.substring(1);
    const params = new URLSearchParams(hash);

    const token = params.get('token');
    const refreshToken = params.get('refresh_token');
    const userStr = params.get('user');

    if (!token || !userStr) {
      setError('OAuth2 login failed. Missing authentication data.');
      return;
    }

    try {
      const user = JSON.parse(decodeURIComponent(userStr));
      setSession(token, refreshToken || undefined, user);

      // Redirect based on role
      navigate(user.role === 'employee' ? '/portal' : '/', { replace: true });
    } catch {
      setError('Failed to process OAuth2 response.');
    }
  }, [navigate, setSession]);

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100">
        <div className="bg-white rounded-2xl shadow p-8 max-w-md w-full text-center space-y-4">
          <div className="w-16 h-16 bg-red-50 rounded-full flex items-center justify-center mx-auto">
            <svg className="w-8 h-8 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
          <p className="text-sm text-gray-600">{error}</p>
          <a href="/login" className="inline-block text-sm text-black font-medium hover:underline">
            Back to login
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100">
      <div className="text-gray-500">Completing sign in...</div>
    </div>
  );
}
