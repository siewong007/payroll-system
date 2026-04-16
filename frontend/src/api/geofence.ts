import api from './client';

export interface CompanyLocation {
  id: string;
  company_id: string;
  name: string;
  latitude: number;
  longitude: number;
  radius_meters: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export function listLocations(): Promise<CompanyLocation[]> {
  return api.get('/geofence/locations').then(r => r.data);
}

export function createLocation(data: {
  name: string;
  latitude: number;
  longitude: number;
  radius_meters?: number;
}): Promise<CompanyLocation> {
  return api.post('/geofence/locations', data).then(r => r.data);
}

export function updateLocation(id: string, data: {
  name?: string;
  latitude?: number;
  longitude?: number;
  radius_meters?: number;
  is_active?: boolean;
}): Promise<CompanyLocation> {
  return api.put(`/geofence/locations/${id}`, data).then(r => r.data);
}

export function deleteLocation(id: string): Promise<void> {
  return api.delete(`/geofence/locations/${id}`).then(r => r.data);
}

export function getGeofenceMode(): Promise<{ mode: string }> {
  return api.get('/geofence/mode').then(r => r.data);
}

export function setGeofenceMode(mode: string): Promise<void> {
  return api.put('/geofence/mode', { mode }).then(r => r.data);
}
