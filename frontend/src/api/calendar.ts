import api from './client';
import type {
  Holiday,
  CreateHolidayRequest,
  UpdateHolidayRequest,
  WorkingDayConfig,
  UpdateWorkingDaysRequest,
  MonthCalendar,
} from '@/types';

export const getHolidays = (year?: number) =>
  api.get<Holiday[]>('/calendar/holidays', { params: { year } }).then(r => r.data);

export const getHoliday = (id: string) =>
  api.get<Holiday>(`/calendar/holidays/${id}`).then(r => r.data);

export const createHoliday = (data: CreateHolidayRequest) =>
  api.post<Holiday>('/calendar/holidays', data).then(r => r.data);

export const updateHoliday = (id: string, data: UpdateHolidayRequest) =>
  api.put<Holiday>(`/calendar/holidays/${id}`, data).then(r => r.data);

export const deleteHoliday = (id: string) =>
  api.delete(`/calendar/holidays/${id}`).then(r => r.data);

export const getWorkingDays = () =>
  api.get<WorkingDayConfig[]>('/calendar/working-days').then(r => r.data);

export const updateWorkingDays = (data: UpdateWorkingDaysRequest) =>
  api.put<WorkingDayConfig[]>('/calendar/working-days', data).then(r => r.data);

export const getMonthCalendar = (year: number, month: number) =>
  api.get<MonthCalendar>('/calendar/month', { params: { year, month } }).then(r => r.data);

export const importIcs = (url: string) =>
  api.post<Holiday[]>('/calendar/import-ics', { url }).then(r => r.data);

export const importIcsFile = (file: File) => {
  const formData = new FormData();
  formData.append('file', file);
  return api.post<Holiday[]>('/calendar/import-ics-file', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  }).then(r => r.data);
};
