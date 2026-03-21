import api from './client';

export interface Notification {
  id: string;
  user_id: string;
  company_id: string;
  notification_type: string;
  title: string;
  message: string;
  entity_type: string | null;
  entity_id: string | null;
  is_read: boolean;
  read_at: string | null;
  created_at: string;
}

export interface NotificationCount {
  unread: number;
  total: number;
}

export const getNotifications = (unreadOnly = false, limit = 50) =>
  api.get<Notification[]>('/notifications', { params: { unread_only: unreadOnly, limit } }).then(r => r.data);

export const getNotificationCount = () =>
  api.get<NotificationCount>('/notifications/count').then(r => r.data);

export const markAsRead = (id: string) =>
  api.put(`/notifications/${id}/read`).then(r => r.data);

export const markAllRead = () =>
  api.put('/notifications/read-all').then(r => r.data);
