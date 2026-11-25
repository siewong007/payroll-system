import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { Bell, Check, CheckCheck, ChevronRight } from 'lucide-react';
import { getNotifications, markAsRead, markAllRead, type Notification } from '@/api/notifications';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';
import { formatDateTime } from '@/lib/format';

function notificationTarget(n: Notification): string | null {
  const kind = (n.entity_type || n.notification_type || '').toLowerCase();
  if (kind.includes('leave')) return '/portal/leave';
  if (kind.includes('claim') || kind.includes('expense')) return '/portal/claims';
  if (kind.includes('overtime') || kind.includes('ot_')) return '/portal/overtime';
  if (kind.includes('payroll') || kind.includes('payslip')) return '/portal/payslips';
  if (kind.includes('attendance')) return '/portal/attendance';
  return null;
}

export function Notifications() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<'all' | 'unread'>('all');

  const { data: notifications = [], isLoading } = useQuery({
    queryKey: ['notifications'],
    queryFn: () => getNotifications(false, 100),
  });

  const markReadM = useMutation({
    mutationFn: markAsRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
      queryClient.invalidateQueries({ queryKey: ['notification-count'] });
    },
  });

  const markAllM = useMutation({
    mutationFn: markAllRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
      queryClient.invalidateQueries({ queryKey: ['notification-count'] });
    },
  });

  const typeStyles: Record<string, string> = {
    leave_approved: 'text-emerald-600 dark:text-emerald-300 bg-emerald-50 dark:bg-emerald-500/15',
    leave_rejected: 'text-red-600 dark:text-red-300 bg-red-50 dark:bg-red-500/15',
    leave_submitted: 'text-gray-900 dark:text-slate-200 bg-gray-100 dark:bg-slate-800',
    claim_approved: 'text-emerald-600 dark:text-emerald-300 bg-emerald-50 dark:bg-emerald-500/15',
    claim_rejected: 'text-red-600 dark:text-red-300 bg-red-50 dark:bg-red-500/15',
    claim_submitted: 'text-gray-900 dark:text-slate-200 bg-gray-100 dark:bg-slate-800',
    overtime_approved: 'text-emerald-600 dark:text-emerald-300 bg-emerald-50 dark:bg-emerald-500/15',
    overtime_rejected: 'text-red-600 dark:text-red-300 bg-red-50 dark:bg-red-500/15',
    overtime_submitted: 'text-gray-900 dark:text-slate-200 bg-gray-100 dark:bg-slate-800',
    payroll_processed: 'text-violet-600 dark:text-violet-300 bg-violet-50 dark:bg-violet-500/15',
    general: 'text-gray-600 dark:text-slate-300 bg-gray-100 dark:bg-slate-800',
  };

  const unreadCount = notifications.filter((n: Notification) => !n.is_read).length;
  const visibleNotifications =
    filter === 'unread' ? notifications.filter((n: Notification) => !n.is_read) : notifications;

  const handleOpenNotification = (n: Notification) => {
    if (!n.is_read) {
      markReadM.mutate(n.id);
    }
    const target = notificationTarget(n);
    if (target) {
      navigate(target);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 dark:border-slate-200" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">{t('portal.notifications.title')}</h1>
          {unreadCount > 0 && <p className="page-subtitle">{t('portal.notifications.unread', { count: unreadCount })}</p>}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-0.5">
            <button
              type="button"
              onClick={() => setFilter('all')}
              aria-pressed={filter === 'all'}
              className={`min-h-9 px-3 rounded-lg text-xs font-medium transition-colors ${
                filter === 'all'
                  ? 'bg-gray-900 dark:bg-teal-600 text-white'
                  : 'text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white'
              }`}
            >
              {t('portal.notifications.allTab')}
            </button>
            <button
              type="button"
              onClick={() => setFilter('unread')}
              aria-pressed={filter === 'unread'}
              className={`min-h-9 px-3 rounded-lg text-xs font-medium transition-colors ${
                filter === 'unread'
                  ? 'bg-gray-900 dark:bg-teal-600 text-white'
                  : 'text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white'
              }`}
            >
              {t('portal.notifications.unreadTab')}
            </button>
          </div>
          {unreadCount > 0 && (
            <button type="button" onClick={() => markAllM.mutate()} disabled={markAllM.isPending} className="btn-secondary">
              <CheckCheck className="w-4 h-4" /> {t('portal.notifications.markAllRead')}
            </button>
          )}
        </div>
      </div>

      <NotificationActivationCard />

      {visibleNotifications.length === 0 ? (
        <div className="card text-center py-16">
          <Bell className="w-12 h-12 mx-auto text-gray-300 dark:text-slate-700 mb-4" />
          <p className="text-gray-500 dark:text-slate-400">
            {filter === 'unread' ? t('portal.notifications.emptyUnread') : t('portal.notifications.empty')}
          </p>
        </div>
      ) : (
        <div className="card !p-0 divide-y divide-gray-100 dark:divide-slate-800 overflow-hidden">
          {visibleNotifications.map((n: Notification) => {
            const target = notificationTarget(n);
            return (
              <div
                key={n.id}
                className={`flex items-start gap-3 px-4 sm:px-5 py-4 transition-all-fast ${
                  n.is_read
                    ? 'bg-white dark:bg-slate-900'
                    : 'bg-teal-50/40 dark:bg-teal-500/10'
                }`}
              >
                <button
                  type="button"
                  onClick={() => handleOpenNotification(n)}
                  className="flex-1 flex items-start gap-3.5 text-left min-w-0 rounded-lg focus:outline-none focus:ring-2 focus:ring-teal-500/40"
                >
                  <div className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 mt-0.5 ${
                    typeStyles[n.notification_type] || typeStyles.general
                  }`}>
                    <Bell className="w-4 h-4" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <p className={`text-sm truncate ${n.is_read ? 'text-gray-700 dark:text-slate-200' : 'text-gray-900 dark:text-white font-semibold'}`}>
                        {n.title}
                      </p>
                      {target && (
                        <ChevronRight className="w-3.5 h-3.5 text-gray-400 dark:text-slate-500 shrink-0" aria-hidden="true" />
                      )}
                    </div>
                    <p className="text-sm text-gray-600 dark:text-slate-300 mt-0.5 break-words">{n.message}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-1.5">
                      {formatDateTime(n.created_at)}
                    </p>
                  </div>
                </button>
                {!n.is_read && (
                  <button
                    type="button"
                    onClick={() => markReadM.mutate(n.id)}
                    aria-label={t('portal.notifications.markRead')}
                    className="shrink-0 min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
                    title={t('portal.notifications.markRead')}
                  >
                    <Check className="w-4 h-4" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
