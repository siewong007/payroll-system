import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe, Laptop, LogOut, Monitor, ShieldCheck, Smartphone, Tablet } from 'lucide-react';
import {
  getSessions,
  revokeOtherSessions,
  revokeSession,
  type UserSession,
} from '@/api/sessions';
import { deviceLabel, parseUserAgent } from '@/lib/userAgent';
import { formatDate, formatRelativeTime } from '@/lib/utils';
import { Modal } from '@/components/ui/Modal';

const DEVICE_ICONS = { desktop: Monitor, mobile: Smartphone, tablet: Tablet, unknown: Globe } as const;

function lastActive(session: UserSession, t: (key: string) => string): string {
  if (Date.now() - new Date(session.last_seen_at).getTime() < 120_000) return t('auth.sessions.activeNow');
  return formatRelativeTime(session.last_seen_at);
}

export function SessionManagement() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [confirmingOthers, setConfirmingOthers] = useState(false);
  const { data: sessions = [], isLoading } = useQuery({ queryKey: ['sessions'], queryFn: getSessions });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['sessions'] });
  const revoke = useMutation({ mutationFn: revokeSession, onSuccess: invalidate });
  const revokeOthers = useMutation({
    mutationFn: revokeOtherSessions,
    onSuccess: () => {
      setConfirmingOthers(false);
      invalidate();
    },
  });
  const otherSessions = sessions.filter((session) => !session.current);

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-gray-400" />
          <span className="section-title">{t('auth.sessions.title')}</span>
        </div>
        {otherSessions.length > 0 && (
          <button
            type="button"
            onClick={() => setConfirmingOthers(true)}
            className="ml-auto text-sm font-medium text-red-600 hover:text-red-700 whitespace-nowrap"
          >
            {t('auth.sessions.signOutOthers')}
          </button>
        )}
      </div>

      <p className="text-sm text-gray-500 mb-4">
        {t('auth.sessions.description')}
      </p>

      {isLoading ? (
        <div className="space-y-3" aria-hidden>
          {[0, 1].map((i) => (
            <div key={i} className="flex items-center gap-3 py-2">
              <div className="w-9 h-9 rounded-xl bg-gray-100" />
              <div className="flex-1 space-y-2">
                <div className="h-3 w-40 rounded bg-gray-100" />
                <div className="h-2.5 w-56 rounded bg-gray-50" />
              </div>
            </div>
          ))}
        </div>
      ) : sessions.length === 0 ? (
        <p className="py-3 text-sm text-gray-500">{t('auth.sessions.empty')}</p>
      ) : (
        <div className="divide-y divide-gray-100">
          {sessions.map((session) => {
            const parsed = parseUserAgent(session.user_agent);
            const Icon = session.current ? Laptop : DEVICE_ICONS[parsed.deviceType];
            return (
              <div key={session.id} className="py-4 flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl bg-[var(--accent-soft)] flex items-center justify-center shrink-0">
                  <Icon className="w-[18px] h-[18px] text-gray-600" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap gap-2 items-center">
                    <span className="text-sm font-medium text-gray-900">{deviceLabel(parsed)}</span>
                    {session.current && (
                      <span className="badge badge-approved inline-flex items-center gap-1.5">
                        <span className="glow-dot" /> {t('auth.sessions.thisDevice')}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-gray-500 mt-1">
                    {session.ip_address ?? t('auth.sessions.ipUnknown')} · {t('auth.sessions.signedIn', { date: formatDate(session.created_at) })} · {lastActive(session, t)}
                  </p>
                </div>
                {!session.current && (
                  <button
                    type="button"
                    aria-label={t('auth.sessions.signOutDevice')}
                    onClick={() => revoke.mutate(session.id)}
                    disabled={revoke.isPending}
                    className="p-2 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded-lg transition-all-fast disabled:opacity-50"
                  >
                    <LogOut className="w-4 h-4" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}

      <Modal
        open={confirmingOthers}
        onClose={() => setConfirmingOthers(false)}
        title={t('auth.sessions.signOutTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={() => setConfirmingOthers(false)}>
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={() => revokeOthers.mutate()}
              disabled={revokeOthers.isPending}
              className="btn-primary !min-h-0 !py-2 !bg-none !bg-red-600 hover:!bg-red-700"
            >
              {revokeOthers.isPending
                ? t('auth.sessions.signingOut')
                : t('auth.sessions.signOutSubmit', { count: otherSessions.length })}
            </button>
          </div>
        }
      >
        <p className="text-sm text-gray-600">
          {t('auth.sessions.signOutBody', { count: otherSessions.length })}
        </p>
      </Modal>
    </section>
  );
}
