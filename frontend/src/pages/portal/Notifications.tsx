import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Bell, Check, CheckCheck, ChevronRight } from 'lucide-react';
import { getNotificationCount, getNotifications, markAsRead, markAllRead, type Notification } from '@/api/notifications';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useAuth } from '@/context/AuthContext';
import { formatDateTime } from '@/lib/format';
import { useToast } from '@/lib/toast';
import { userCan } from '@/lib/usePermissions';
import { getErrorMessage } from '@/lib/utils';

function notificationTarget(
  n: Notification,
  canOpenApprovals: boolean,
  hasEmployee: boolean,
): string | null {
  const nType = (n.notification_type || '').toLowerCase();
  if (canOpenApprovals) {
    if (nType === 'leave_submitted') return '/approvals?tab=leave';
    if (nType === 'claim_submitted') return '/approvals?tab=claims';
    if (nType === 'overtime_submitted') return '/approvals?tab=overtime';
  }
  if (!hasEmployee) return null;
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
  const { user } = useAuth();
  const navigate = useNavigate();
  const toast = useToast();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<'all' | 'unread'>('all');
  const [markingId, setMarkingId] = useState<string | null>(null);
  const hasCompany = Boolean(user?.company_id);
  const hasEmployee = Boolean(user?.employee_id);
  const canOpenApprovals = hasCompany && userCan(user, 'view_approvals');

  const {
    data: notifications = [],
    isLoading,
    isFetching,
    isError,
    error,
    refetch,
  } = useQuery({
    queryKey: ['notifications', filter],
    queryFn: () => getNotifications(filter === 'unread', 100),
    enabled: hasCompany,
  });

  const { data: countData } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    enabled: hasCompany,
  });

  const markReadM = useMutation({
    mutationFn: markAsRead,
    onMutate: (id) => setMarkingId(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
      queryClient.invalidateQueries({ queryKey: ['notification-count'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('common.error'))),
    onSettled: () => setMarkingId(null),
  });

  const markAllM = useMutation({
    mutationFn: markAllRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
      queryClient.invalidateQueries({ queryKey: ['notification-count'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('common.error'))),
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
    payroll_processed: 'text-[var(--accent-1)] bg-[var(--accent-soft)]',
    attendance_absent: 'text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-500/15',
    general: 'text-gray-600 dark:text-slate-300 bg-gray-100 dark:bg-slate-800',
  };

  const unreadCount = countData?.unread ?? notifications.filter((n: Notification) => !n.is_read).length;
  const visibleNotifications =
    filter === 'unread' ? notifications.filter((n: Notification) => !n.is_read) : notifications;

  const handleOpenNotification = (n: Notification) => {
    if (!n.is_read) {
      markReadM.mutate(n.id);
    }
    const target = notificationTarget(n, canOpenApprovals, hasEmployee);
    if (target) {
      navigate(target);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        className="mb-0"
        title={t('portal.notifications.title')}
        subtitle={
          unreadCount > 0
            ? t('portal.notifications.unread', { count: unreadCount })
            : undefined
        }
        actions={
          <>
            <div className="inline-flex rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-0.5">
              <button
                type="button"
                onClick={() => setFilter('all')}
                aria-pressed={filter === 'all'}
                className={`min-h-11 sm:min-h-9 px-3 rounded-lg text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                  filter === 'all'
                    ? 'bg-[var(--accent-1)] text-white'
                    : 'text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white'
                }`}
              >
                {t('portal.notifications.allTab')}
              </button>
              <button
                type="button"
                onClick={() => setFilter('unread')}
                aria-pressed={filter === 'unread'}
                className={`min-h-11 sm:min-h-9 px-3 rounded-lg text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                  filter === 'unread'
                    ? 'bg-[var(--accent-1)] text-white'
                    : 'text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white'
                }`}
              >
                {t('portal.notifications.unreadTab')}
              </button>
            </div>
            {unreadCount > 0 && (
              <button type="button" onClick={() => markAllM.mutate()} disabled={markAllM.isPending} className="btn-secondary">
                <CheckCheck className="w-4 h-4" aria-hidden="true" /> {t('portal.notifications.markAllRead')}
              </button>
            )}
          </>
        }
      />

      <NotificationActivationCard compact />

      {isLoading ? (
        <div
          role="status"
          aria-busy="true"
          aria-label={t('common.loading')}
          className="card !p-0 divide-y divide-gray-100 dark:divide-slate-800 overflow-hidden"
        >
          {[0, 1, 2, 3].map((i) => (
            <div key={i} className="flex items-start gap-3 px-4 sm:px-5 py-4">
              <Skeleton className="w-9 h-9 rounded-xl shrink-0" />
              <div className="flex-1 space-y-2">
                <Skeleton className="h-4 w-44" />
                <Skeleton className="h-3.5 w-3/4" />
                <Skeleton className="h-3 w-28" />
              </div>
            </div>
          ))}
        </div>
      ) : isError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('portal.notifications.loadFailed')}
            description={getErrorMessage(error, t('portal.notifications.loadFailed'))}
            action={
              <button type="button" onClick={() => void refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : visibleNotifications.length === 0 ? (
        <div className="card">
          <EmptyState
            icon={Bell}
            title={filter === 'unread' ? t('portal.notifications.emptyUnread') : t('portal.notifications.empty')}
            action={
              filter === 'unread' ? (
                <button type="button" onClick={() => setFilter('all')} className="btn-secondary">
                  {t('portal.notifications.allTab')}
                </button>
              ) : undefined
            }
          />
        </div>
      ) : (
        <div className={`card !p-0 divide-y divide-gray-100 dark:divide-slate-800 overflow-hidden ${isFetching && !isLoading ? 'opacity-75 transition-opacity' : ''}`}>
          {visibleNotifications.map((n: Notification) => {
            const target = notificationTarget(n, canOpenApprovals, hasEmployee);
            const isInteractive = !n.is_read || Boolean(target);
            const content = (
              <>
                <div className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 mt-0.5 ${
                  typeStyles[n.notification_type] || typeStyles.general
                }`}>
                  <Bell className="w-4 h-4" aria-hidden="true" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-start gap-2">
                    <p className={`text-sm line-clamp-2 ${n.is_read ? 'text-gray-700 dark:text-slate-200' : 'text-gray-900 dark:text-white font-semibold'}`}>
                      {n.title}
                    </p>
                    {target && (
                      <ChevronRight className="w-3.5 h-3.5 text-gray-400 dark:text-slate-500 shrink-0 mt-1" aria-hidden="true" />
                    )}
                  </div>
                  <p className="text-sm text-gray-600 dark:text-slate-300 mt-0.5 break-words">{n.message}</p>
                  <p className="text-xs text-gray-500 dark:text-slate-400 mt-1.5 tabular-nums">
                    {formatDateTime(n.created_at)}
                  </p>
                </div>
              </>
            );
            return (
              <div
                key={n.id}
                className={`flex items-start gap-3 px-4 sm:px-5 py-4 transition-all-fast ${
                  n.is_read
                    ? 'bg-white dark:bg-slate-900'
                    : 'bg-[var(--accent-soft)]'
                }`}
              >
                {isInteractive ? (
                  <button
                    type="button"
                    onClick={() => handleOpenNotification(n)}
                    className="flex-1 flex items-start gap-3.5 text-left min-w-0 rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                  >
                    {content}
                  </button>
                ) : (
                  <div className="flex-1 flex items-start gap-3.5 text-left min-w-0">
                    {content}
                  </div>
                )}
                {!n.is_read && (
                  <button
                    type="button"
                    disabled={markingId === n.id && markReadM.isPending}
                    onClick={() => markReadM.mutate(n.id)}
                    aria-label={`${t('portal.notifications.markRead')} — ${n.title}`}
                    className="shrink-0 min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg disabled:opacity-50 transition-all-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                    title={t('portal.notifications.markRead')}
                  >
                    <Check className="w-4 h-4" aria-hidden="true" />
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
