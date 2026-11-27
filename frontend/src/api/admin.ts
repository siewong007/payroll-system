import api from './client';
import type {
  Company,
  CreateCompanyRequest,
  UpdateCompanyRequest,
  User,
  UserWithCompanies,
  CreateUserRequest,
  UpdateUserRequest,
  ListUsersParams,
  PaginatedResponse,
  CompanySummary,
  LoginResponse,
  StepUpCredentials,
} from '@/types';

// Company management (super_admin)
export const listCompanies = () =>
  api.get<Company[]>('/admin/companies').then((r) => r.data);

export const createCompany = (req: CreateCompanyRequest) =>
  api.post<Company>('/admin/companies', req).then((r) => r.data);

export const updateCompanyAdmin = (id: string, req: UpdateCompanyRequest) =>
  api.put<Company>(`/admin/companies/${id}`, req).then((r) => r.data);

export const deleteCompany = (id: string, stepUp: StepUpCredentials) =>
  api.delete<{ ok: boolean; token?: string; user?: User }>(`/admin/companies/${id}`, { data: stepUp }).then((r) => r.data);

// User management
export const listUsers = (params: ListUsersParams = {}) =>
  api.get<PaginatedResponse<UserWithCompanies>>('/admin/users', {
    params: {
      company_id: params.companyId,
      role: params.role || undefined,
      search: params.search?.trim() || undefined,
      page: params.page,
      per_page: params.perPage,
    },
  }).then((r) => r.data);

export const createUser = (req: CreateUserRequest) =>
  api.post<UserWithCompanies>('/admin/users', req).then((r) => r.data);

export const updateUser = (userId: string, req: UpdateUserRequest) =>
  api.put<UserWithCompanies>(`/admin/users/${userId}`, req).then((r) => r.data);

export const deleteUser = (userId: string, stepUp: StepUpCredentials) =>
  api.delete(`/admin/users/${userId}`, { data: stepUp }).then((r) => r.data);

export const resetUser2fa = (userId: string, password: string, totpCode?: string) =>
  api
    .post<{ ok: boolean }>(`/admin/users/${userId}/2fa/reset`, {
      password,
      ...(totpCode ? { totp_code: totpCode } : {}),
    })
    .then((r) => r.data);

// Company switching
export const getMyCompanies = () =>
  api.get<CompanySummary[]>('/auth/my-companies').then((r) => r.data);

export const switchCompany = (companyId: string) =>
  api.put<LoginResponse>('/auth/switch-company', { company_id: companyId }).then((r) => r.data);

// Auth - forgot/reset password (public, Turnstile-gated server-side)
export const forgotPassword = (email: string, turnstileToken?: string) =>
  api.post('/auth/forgot-password', { email, turnstile_token: turnstileToken }).then((r) => r.data);

export const resetPassword = (token: string, new_password: string, turnstileToken?: string) =>
  api
    .post('/auth/reset-password', { token, new_password, turnstile_token: turnstileToken })
    .then((r) => r.data);

export const validateResetToken = (token: string) =>
  api.post('/auth/validate-reset-token', { token }).then((r) => r.data);
