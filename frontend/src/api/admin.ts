import api from './client';
import type {
  Company,
  CreateCompanyRequest,
  UpdateCompanyRequest,
  UserWithCompanies,
  CreateUserRequest,
  UpdateUserRequest,
  UpdateUserCompaniesRequest,
  CompanySummary,
  LoginResponse,
} from '@/types';

// Company management (super_admin)
export const listCompanies = () =>
  api.get<Company[]>('/admin/companies').then((r) => r.data);

export const createCompany = (req: CreateCompanyRequest) =>
  api.post<Company>('/admin/companies', req).then((r) => r.data);

export const updateCompanyAdmin = (id: string, req: UpdateCompanyRequest) =>
  api.put<Company>(`/admin/companies/${id}`, req).then((r) => r.data);

// User management
export const listUsers = () =>
  api.get<UserWithCompanies[]>('/admin/users').then((r) => r.data);

export const createUser = (req: CreateUserRequest) =>
  api.post<UserWithCompanies>('/admin/users', req).then((r) => r.data);

export const updateUser = (userId: string, req: UpdateUserRequest) =>
  api.put<UserWithCompanies>(`/admin/users/${userId}`, req).then((r) => r.data);

export const deleteUser = (userId: string) =>
  api.delete(`/admin/users/${userId}`).then((r) => r.data);

export const updateUserCompanies = (userId: string, req: UpdateUserCompaniesRequest) =>
  api.put<UserWithCompanies>(`/admin/users/${userId}/companies`, req).then((r) => r.data);

// Company switching
export const getMyCompanies = () =>
  api.get<CompanySummary[]>('/auth/my-companies').then((r) => r.data);

export const switchCompany = (companyId: string) =>
  api.put<LoginResponse>('/auth/switch-company', { company_id: companyId }).then((r) => r.data);

// Auth - forgot/reset password (public)
export const forgotPassword = (email: string) =>
  api.post('/auth/forgot-password', { email }).then((r) => r.data);

export const resetPassword = (token: string, new_password: string) =>
  api.post('/auth/reset-password', { token, new_password }).then((r) => r.data);

export const validateResetToken = (token: string) =>
  api.post('/auth/validate-reset-token', { token }).then((r) => r.data);

// Password reset management (admin)
export const listPasswordResets = () =>
  api.get('/admin/password-resets').then((r) => r.data);

export const approvePasswordReset = (id: string) =>
  api.post<{ reset_url: string }>(`/admin/password-resets/${id}/approve`).then((r) => r.data);

export const rejectPasswordReset = (id: string) =>
  api.post(`/admin/password-resets/${id}/reject`).then((r) => r.data);
