import api from './client';

export interface WorkSchedule {
  id: string;
  company_id: string;
  name: string;
  start_time: string; // "HH:MM:SS"
  end_time: string;
  grace_minutes: number;
  half_day_hours: number;
  timezone: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpsertWorkScheduleRequest {
  name?: string;
  start_time: string; // "HH:MM"
  end_time: string;
  grace_minutes?: number;
  half_day_hours?: number;
  timezone?: string;
}

export interface UpdateWorkScheduleRequest {
  name?: string;
  start_time?: string;
  end_time?: string;
  grace_minutes?: number;
  half_day_hours?: number;
  timezone?: string;
}

export function listWorkSchedules(): Promise<WorkSchedule[]> {
  return api.get('/work-schedules').then(r => r.data);
}

export function getDefaultSchedule(): Promise<{ schedule: WorkSchedule | null }> {
  return api.get('/work-schedules/default').then(r => r.data);
}

export function upsertDefaultSchedule(data: UpsertWorkScheduleRequest): Promise<WorkSchedule> {
  return api.put('/work-schedules/default', data).then(r => r.data);
}

export function updateWorkSchedule(id: string, data: UpdateWorkScheduleRequest): Promise<WorkSchedule> {
  return api.put(`/work-schedules/${id}`, data).then(r => r.data);
}
