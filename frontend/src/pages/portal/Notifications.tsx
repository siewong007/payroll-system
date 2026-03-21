import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Bell, Check, CheckCheck } from 'lucide-react';
import { getNotifications, markAsRead, markAllRead, type Notification } from '@/api/notifications';

export function Notifications() {
  const queryClient = useQueryClient();

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
    leave_approved: 'text-emerald-600 bg-emerald-50',
    leave_rejected: 'text-red-600 bg-red-50',
    leave_submitted: 'text-gray-900 bg-gray-100',
    claim_approved: 'text-emerald-600 bg-emerald-50',
    claim_rejected: 'text-red-600 bg-red-50',
    claim_submitted: 'text-gray-900 bg-gray-100',
    payroll_processed: 'text-violet-600 bg-violet-50',
    general: 'text-gray-600 bg-gray-50',
  };

  const unreadCount = notifications.filter((n: Notification) => !n.is_read).length;

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="page-header">
          <h1 className="page-title">Notifications</h1>
          {unreadCount > 0 && <p className="page-subtitle">{unreadCount} unread</p>}
        </div>
        {unreadCount > 0 && (
          <button onClick={() => markAllM.mutate()} disabled={markAllM.isPending} className="btn-secondary">
            <CheckCheck className="w-4 h-4" /> Mark all read
          </button>
        )}
      </div>

      {notifications.length === 0 ? (
        <div className="card text-center py-16">
          <Bell className="w-12 h-12 mx-auto text-gray-200 mb-4" />
          <p className="text-gray-400">No notifications yet</p>
        </div>
      ) : (
        <div className="card p-0 divide-y divide-gray-100 overflow-hidden">
          {notifications.map((n: Notification) => (
            <div
              key={n.id}
              className={`flex items-start gap-4 px-5 py-4 transition-all-fast ${
                n.is_read ? 'bg-white' : 'bg-gray-50/30'
              }`}
            >
              <div className={`w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 mt-0.5 ${
                typeStyles[n.notification_type] || typeStyles.general
              }`}>
                <Bell className="w-4 h-4" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <p className={`text-sm ${n.is_read ? 'text-gray-600' : 'text-gray-900 font-semibold'}`}>
                      {n.title}
                    </p>
                    <p className="text-sm text-gray-400 mt-0.5">{n.message}</p>
                  </div>
                  {!n.is_read && (
                    <button
                      onClick={() => markReadM.mutate(n.id)}
                      className="flex-shrink-0 p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-all-fast"
                      title="Mark as read"
                    >
                      <Check className="w-4 h-4" />
                    </button>
                  )}
                </div>
                <p className="text-xs text-gray-300 mt-1.5">
                  {new Date(n.created_at).toLocaleString()}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
