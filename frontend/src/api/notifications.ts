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

export interface PushConfig {
  enabled: boolean;
  vapid_public_key: string | null;
}

export interface PushSubscriptionPayload {
  endpoint: string;
  keys: {
    p256dh: string;
    auth: string;
  };
}

export interface PushSubscriptionStatus {
  subscribed: boolean;
  active_subscriptions: number;
}

export interface TestPushResult {
  queued: number;
  sent: number;
  failed: number;
  gone: number;
}

export const getNotifications = (unreadOnly = false, limit = 50) =>
  api
    .get<Notification[]>('/notifications', { params: { unread_only: unreadOnly, limit } })
    .then((r) => r.data);

export const getNotificationCount = () =>
  api.get<NotificationCount>('/notifications/count').then((r) => r.data);

export const markAsRead = (id: string) =>
  api.put(`/notifications/${id}/read`).then((r) => r.data);

export const markAllRead = () =>
  api.put('/notifications/read-all').then((r) => r.data);

export const getPushConfig = () =>
  api.get<PushConfig>('/notifications/push-config').then((r) => r.data);

export const savePushSubscription = (payload: PushSubscriptionPayload) =>
  api.post<PushSubscriptionStatus>('/notifications/subscriptions', payload).then((r) => r.data);

export const deletePushSubscription = (endpoint: string) =>
  api
    .delete<PushSubscriptionStatus>('/notifications/subscriptions', { data: { endpoint } })
    .then((r) => r.data);

export const sendTestPush = () =>
  api.post<TestPushResult>('/notifications/test-push').then((r) => r.data);
