import api from './client';
import type { CompanySetting, SettingUpdate } from '@/types';

export async function getSettings(category?: string): Promise<CompanySetting[]> {
  const { data } = await api.get('/settings', { params: category ? { category } : undefined });
  return data;
}

export async function getSetting(category: string, key: string): Promise<CompanySetting> {
  const { data } = await api.get(`/settings/${category}/${key}`);
  return data;
}

export async function updateSetting(category: string, key: string, value: unknown): Promise<CompanySetting> {
  const { data } = await api.put(`/settings/${category}/${key}`, { value });
  return data;
}

export async function bulkUpdateSettings(settings: SettingUpdate[]): Promise<CompanySetting[]> {
  const { data } = await api.put('/settings', { settings });
  return data;
}
