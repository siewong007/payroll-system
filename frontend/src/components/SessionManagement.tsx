import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Laptop, LogOut, ShieldCheck } from 'lucide-react';
import { getSessions, revokeOtherSessions, revokeSession } from '@/api/sessions';

function displayDevice(userAgent: string | null) {
  if (!userAgent) return 'Unknown device';
  if (userAgent.includes('iPhone') || userAgent.includes('Android')) return 'Mobile device';
  if (userAgent.includes('Macintosh')) return 'Mac computer';
  if (userAgent.includes('Windows')) return 'Windows computer';
  return 'Browser session';
}

export function SessionManagement() {
  const queryClient = useQueryClient();
  const { data: sessions = [], isLoading } = useQuery({ queryKey: ['sessions'], queryFn: getSessions });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['sessions'] });
  const revoke = useMutation({ mutationFn: revokeSession, onSuccess: invalidate });
  const revokeOthers = useMutation({ mutationFn: revokeOtherSessions, onSuccess: invalidate });
  const otherSessions = sessions.filter((session) => !session.current);

  return (
    <section className="bg-white rounded-2xl shadow p-6">
      <div className="flex items-start justify-between gap-4 mb-5">
        <div>
          <div className="flex items-center gap-2 text-gray-900 font-semibold"><ShieldCheck className="w-5 h-5" /> Active sessions</div>
          <p className="text-sm text-gray-500 mt-1">Sign out devices you no longer use. Revoked devices will need to sign in again.</p>
        </div>
        {otherSessions.length > 0 && (
          <button
            type="button"
            onClick={() => { if (window.confirm('Sign out all other devices?')) revokeOthers.mutate(); }}
            disabled={revokeOthers.isPending}
            className="text-sm font-medium text-red-600 hover:text-red-700 disabled:opacity-50 whitespace-nowrap"
          >
            Sign out all others
          </button>
        )}
      </div>

      {isLoading ? <div className="text-sm text-gray-400">Loading sessions…</div> : (
        <div className="divide-y divide-gray-100">
          {sessions.map((session) => (
            <div key={session.id} className="py-4 flex items-center gap-3">
              <Laptop className="w-5 h-5 text-gray-400 shrink-0" />
              <div className="min-w-0 flex-1">
                <div className="flex gap-2 items-center"><span className="text-sm font-medium text-gray-900">{displayDevice(session.user_agent)}</span>{session.current && <span className="text-xs bg-green-100 text-green-700 px-2 py-0.5 rounded-full">This device</span>}</div>
                <p className="text-xs text-gray-500 mt-1">Last active {new Date(session.last_seen_at).toLocaleString()}</p>
              </div>
              {!session.current && (
                <button type="button" aria-label="Sign out device" onClick={() => revoke.mutate(session.id)} disabled={revoke.isPending} className="p-2 text-gray-400 hover:text-red-600 disabled:opacity-50"><LogOut className="w-4 h-4" /></button>
              )}
            </div>
          ))}
          {!sessions.length && <p className="py-3 text-sm text-gray-500">No active sessions found.</p>}
        </div>
      )}
    </section>
  );
}
