import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { listPasswordResets, approvePasswordReset, rejectPasswordReset } from '@/api/admin';
import type { PasswordResetRequest } from '@/types';

const statusColors: Record<string, string> = {
  pending: 'bg-yellow-100 text-yellow-800',
  approved: 'bg-blue-100 text-blue-800',
  rejected: 'bg-red-100 text-red-800',
  completed: 'bg-green-100 text-green-800',
  expired: 'bg-gray-100 text-gray-600',
};

export function PasswordResets() {
  const queryClient = useQueryClient();
  const [resetToken, setResetToken] = useState<string | null>(null);
  const [copiedToken, setCopiedToken] = useState(false);

  const { data: requests = [], isLoading } = useQuery({
    queryKey: ['password-resets'],
    queryFn: listPasswordResets,
  });

  const approveMutation = useMutation({
    mutationFn: approvePasswordReset,
    onSuccess: (data) => {
      setResetToken(data.reset_token);
      queryClient.invalidateQueries({ queryKey: ['password-resets'] });
    },
  });

  const rejectMutation = useMutation({
    mutationFn: rejectPasswordReset,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['password-resets'] });
    },
  });

  const copyResetLink = () => {
    if (resetToken) {
      const link = `${window.location.origin}/reset-password?token=${resetToken}`;
      navigator.clipboard.writeText(link);
      setCopiedToken(true);
      setTimeout(() => setCopiedToken(false), 2000);
    }
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString('en-MY', {
      day: '2-digit',
      month: 'short',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const pendingRequests = requests.filter((r: PasswordResetRequest) => r.status === 'pending');
  const otherRequests = requests.filter((r: PasswordResetRequest) => r.status !== 'pending');

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-gray-900">Password Reset Requests</h1>
          <p className="text-sm text-gray-500 mt-1">
            Review and approve password reset requests from users
          </p>
        </div>
        {pendingRequests.length > 0 && (
          <span className="bg-yellow-100 text-yellow-800 text-sm font-medium px-3 py-1 rounded-full">
            {pendingRequests.length} pending
          </span>
        )}
      </div>

      {/* Reset Token Modal */}
      {resetToken && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          className="bg-blue-50 border border-blue-200 rounded-xl p-5"
        >
          <div className="flex items-start justify-between">
            <div className="flex-1">
              <h3 className="text-sm font-semibold text-blue-900">Reset Link Generated</h3>
              <p className="text-xs text-blue-700 mt-1">
                Copy the link below and share it securely with the user. This link expires in 24 hours.
              </p>
              <div className="mt-3 flex items-center gap-2">
                <code className="text-xs bg-white px-3 py-2 rounded-lg border border-blue-200 flex-1 break-all">
                  {window.location.origin}/reset-password?token={resetToken}
                </code>
                <button
                  onClick={copyResetLink}
                  className="px-3 py-2 bg-blue-600 text-white text-xs rounded-lg hover:bg-blue-700 whitespace-nowrap"
                >
                  {copiedToken ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>
            <button
              onClick={() => setResetToken(null)}
              className="text-blue-400 hover:text-blue-600 ml-3"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </motion.div>
      )}

      {isLoading ? (
        <div className="text-center py-12 text-gray-400">Loading...</div>
      ) : requests.length === 0 ? (
        <div className="bg-white rounded-xl shadow-sm border p-12 text-center">
          <p className="text-gray-400">No password reset requests</p>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Pending Requests */}
          {pendingRequests.length > 0 && (
            <div>
              <h2 className="text-sm font-semibold text-gray-700 mb-3">Pending Approval</h2>
              <div className="space-y-3">
                {pendingRequests.map((req: PasswordResetRequest) => (
                  <motion.div
                    key={req.id}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className="bg-white rounded-xl shadow-sm border border-yellow-200 p-5"
                  >
                    <div className="flex items-center justify-between">
                      <div>
                        <p className="text-sm font-medium text-gray-900">{req.user_full_name}</p>
                        <p className="text-xs text-gray-500">{req.user_email}</p>
                        <div className="flex items-center gap-2 mt-1">
                          <span className="text-xs text-gray-400">
                            Requested {formatDate(req.requested_at)}
                          </span>
                          <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600">
                            {req.user_role}
                          </span>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => rejectMutation.mutate(req.id)}
                          disabled={rejectMutation.isPending}
                          className="px-4 py-2 text-sm border border-gray-300 rounded-lg text-gray-600 hover:bg-gray-50 disabled:opacity-50"
                        >
                          Reject
                        </button>
                        <button
                          onClick={() => approveMutation.mutate(req.id)}
                          disabled={approveMutation.isPending}
                          className="px-4 py-2 text-sm bg-black text-white rounded-lg hover:bg-gray-800 disabled:opacity-50"
                        >
                          Approve
                        </button>
                      </div>
                    </div>
                  </motion.div>
                ))}
              </div>
            </div>
          )}

          {/* History */}
          {otherRequests.length > 0 && (
            <div>
              <h2 className="text-sm font-semibold text-gray-700 mb-3">History</h2>
              <div className="bg-white rounded-xl shadow-sm border overflow-hidden">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b bg-gray-50">
                      <th className="text-left px-4 py-3 font-medium text-gray-500">User</th>
                      <th className="text-left px-4 py-3 font-medium text-gray-500">Status</th>
                      <th className="text-left px-4 py-3 font-medium text-gray-500">Requested</th>
                      <th className="text-left px-4 py-3 font-medium text-gray-500">Reviewed</th>
                    </tr>
                  </thead>
                  <tbody>
                    {otherRequests.map((req: PasswordResetRequest) => (
                      <tr key={req.id} className="border-b last:border-0">
                        <td className="px-4 py-3">
                          <p className="font-medium text-gray-900">{req.user_full_name}</p>
                          <p className="text-xs text-gray-400">{req.user_email}</p>
                        </td>
                        <td className="px-4 py-3">
                          <span className={`text-xs px-2 py-1 rounded-full font-medium ${statusColors[req.status]}`}>
                            {req.status}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-gray-500 text-xs">
                          {formatDate(req.requested_at)}
                        </td>
                        <td className="px-4 py-3 text-gray-500 text-xs">
                          {req.reviewed_at ? formatDate(req.reviewed_at) : '-'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
