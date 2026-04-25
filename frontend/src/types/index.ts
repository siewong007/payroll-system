export interface User {
  id: string;
  email: string;
  full_name: string;
  role: 'super_admin' | 'admin' | 'payroll_admin' | 'hr_manager' | 'finance' | 'exec' | 'employee';
  company_id: string | null;
  employee_id: string | null;
  must_change_password?: boolean;
}

export interface LoginResponse {
  token: string;
  refresh_token?: string;
  user: User;
}

// Password Reset
// OAuth2
export interface LinkedOAuth2Account {
  id: string;
  provider: string;
  provider_email: string | null;
  provider_name: string | null;
  avatar_url: string | null;
  linked_at: string;
}

export interface PasswordResetRequest {
  id: string;
  user_id: string;
  status: 'pending' | 'approved' | 'rejected' | 'completed' | 'expired';
  requested_at: string;
  reviewed_by: string | null;
  reviewed_at: string | null;
  completed_at: string | null;
  user_email: string;
  user_full_name: string;
  user_role: string;
}

// Admin - Company Management
export interface CompanySummary {
  id: string;
  name: string;
}

export interface CreateCompanyRequest {
  name: string;
  registration_number?: string;
  tax_number?: string;
  email?: string;
  phone?: string;
}

// Admin - User Management
export interface UserWithCompanies {
  id: string;
  email: string;
  full_name: string;
  role: string;
  company_id: string | null;
  employee_id: string | null;
  is_active: boolean | null;
  created_at: string;
  companies: CompanySummary[];
}

export interface CreateUserRequest {
  email: string;
  password: string;
  full_name: string;
  role: 'super_admin' | 'admin' | 'payroll_admin' | 'hr_manager' | 'finance' | 'exec' | 'employee';
  company_ids: string[];
}

export interface UpdateUserRequest {
  full_name?: string;
  email?: string;
  role?: string;
  is_active?: boolean;
  company_ids?: string[];
}

export interface UpdateUserCompaniesRequest {
  company_ids: string[];
}

export interface Employee {
  id: string;
  company_id: string;
  employee_number: string;
  full_name: string;
  ic_number: string | null;
  passport_number: string | null;
  date_of_birth: string | null;
  gender: string | null;
  nationality: string | null;
  race: string | null;
  residency_status: string;
  marital_status: string | null;
  email: string | null;
  phone: string | null;
  address_line1: string | null;
  address_line2: string | null;
  city: string | null;
  state: string | null;
  postcode: string | null;
  department: string | null;
  designation: string | null;
  cost_centre: string | null;
  branch: string | null;
  employment_type: string;
  date_joined: string;
  probation_start: string | null;
  probation_end: string | null;
  confirmation_date: string | null;
  date_resigned: string | null;
  basic_salary: number; // in sen
  hourly_rate: number | null;
  daily_rate: number | null;
  bank_name: string | null;
  bank_account_number: string | null;
  bank_account_type: string | null;
  tax_identification_number: string | null;
  epf_number: string | null;
  socso_number: string | null;
  eis_number: string | null;
  working_spouse: boolean | null;
  num_children: number | null;
  epf_category: string | null;
  is_muslim: boolean | null;
  zakat_eligible: boolean | null;
  zakat_monthly_amount: number | null;
  ptptn_monthly_amount: number | null;
  tabung_haji_amount: number | null;
  payroll_group_id: string | null;
  salary_group: string | null;
  is_active: boolean | null;
  created_at: string;
  updated_at: string;
}

export interface CreateEmployeeRequest {
  employee_number: string;
  full_name: string;
  ic_number?: string;
  date_of_birth?: string;
  gender?: string;
  nationality?: string;
  race?: string;
  residency_status?: string;
  marital_status?: string;
  email?: string;
  phone?: string;
  address_line1?: string;
  address_line2?: string;
  city?: string;
  state?: string;
  postcode?: string;
  department?: string;
  designation?: string;
  cost_centre?: string;
  branch?: string;
  employment_type?: string;
  date_joined: string;
  basic_salary: number;
  bank_name?: string;
  bank_account_number?: string;
  tax_identification_number?: string;
  epf_number?: string;
  socso_number?: string;
  eis_number?: string;
  working_spouse?: boolean;
  num_children?: number;
  epf_category?: string;
  is_muslim?: boolean;
  zakat_eligible?: boolean;
  zakat_monthly_amount?: number;
  ptptn_monthly_amount?: number;
  payroll_group_id?: string;
  is_active?: boolean;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}

export interface PayrollGroup {
  id: string;
  company_id: string;
  name: string;
  description: string | null;
  cutoff_day: number;
  payment_day: number;
  is_active: boolean;
}

export interface PayrollRun {
  id: string;
  company_id: string;
  payroll_group_id: string;
  period_year: number;
  period_month: number;
  period_start: string;
  period_end: string;
  pay_date: string;
  status: 'draft' | 'processing' | 'processed' | 'pending_approval' | 'approved' | 'paid' | 'cancelled';
  total_gross: number;
  total_net: number;
  total_employer_cost: number;
  total_epf_employee: number;
  total_epf_employer: number;
  total_socso_employee: number;
  total_socso_employer: number;
  total_eis_employee: number;
  total_eis_employer: number;
  total_pcb: number;
  total_zakat: number;
  employee_count: number;
  version: number;
  processed_by: string | null;
  processed_at: string | null;
  approved_by: string | null;
  approved_at: string | null;
  locked_by: string | null;
  locked_at: string | null;
  notes: string | null;
}

export interface PayrollItemSummary {
  employee_id: string;
  employee_name: string;
  employee_number: string;
  basic_salary: number;
  total_allowances: number;
  total_overtime: number;
  total_claims: number;
  gross_salary: number;
  total_deductions: number;
  net_salary: number;
  epf_employee: number;
  socso_employee: number;
  eis_employee: number;
  pcb_amount: number;
}

export interface PayrollSummary {
  payroll_run: PayrollRun;
  items: PayrollItemSummary[];
}

export interface ProcessPayrollRequest {
  payroll_group_id: string;
  period_year: number;
  period_month: number;
  pay_date?: string;
  notes?: string;
}

export interface PayrollEntry {
  id: string;
  employee_id: string;
  company_id: string;
  period_year: number;
  period_month: number;
  category: 'earning' | 'deduction';
  item_type: string;
  description: string;
  amount: number;
  quantity: number | null;
  rate: number | null;
  is_taxable: boolean | null;
  is_processed: boolean | null;
  payroll_run_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface PayrollEntryWithEmployee extends PayrollEntry {
  employee_name: string | null;
  employee_number: string | null;
}

export interface CreatePayrollEntryRequest {
  employee_id: string;
  period_year: number;
  period_month: number;
  category: 'earning' | 'deduction';
  item_type: string;
  description: string;
  amount: number;
  quantity?: number;
  rate?: number;
  is_taxable?: boolean;
}

export type UpdatePayrollEntryRequest = Partial<CreatePayrollEntryRequest>;

export interface UpdatePayrollPcbRequest {
  pcb_amount: number;
}

export interface DashboardSummary {
  total_employees: number;
  active_employees: number;
  last_payroll_period: string | null;
  last_payroll_total_net: number | null;
  last_payroll_total_gross: number | null;
  last_payroll_employee_count: number | null;
  ytd_total_gross: number;
  ytd_total_epf_employer: number;
  ytd_total_socso_employer: number;
  ytd_total_eis_employer: number;
  departments: { department: string; count: number }[];
}

export interface YearMonthsOption {
  year: number;
  months: number[];
}

export interface ReportPeriodsResponse {
  default_year: number;
  default_month: number;
  payroll_years: number[];
  payroll_months: YearMonthsOption[];
  leave_years: number[];
  claims_years: number[];
  ea_form_years: number[];
}

export interface SalaryHistory {
  id: string;
  employee_id: string;
  old_salary: number;
  new_salary: number;
  effective_date: string;
  reason: string | null;
  created_at: string;
}

export interface Tp3Record {
  id: string;
  employee_id: string;
  tax_year: number;
  previous_employer_name: string | null;
  previous_income_ytd: number;
  previous_epf_ytd: number;
  previous_pcb_ytd: number;
  previous_socso_ytd: number;
  previous_zakat_ytd: number;
}

// Documents
export interface DocumentCategory {
  id: string;
  company_id: string;
  name: string;
  description: string | null;
  is_active: boolean;
  created_at: string;
}

export interface Document {
  id: string;
  company_id: string;
  employee_id: string | null;
  category_id: string | null;
  title: string;
  description: string | null;
  file_name: string;
  file_url: string;
  file_size: number | null;
  mime_type: string | null;
  status: 'active' | 'expired' | 'archived';
  issue_date: string | null;
  expiry_date: string | null;
  is_confidential: boolean;
  tags: string | null;
  created_at: string;
  updated_at: string;
  employee_name: string | null;
  employee_number: string | null;
}

export interface CreateDocumentRequest {
  employee_id?: string;
  category_id?: string;
  title: string;
  description?: string;
  file_name: string;
  file_url: string;
  file_size?: number;
  mime_type?: string;
  issue_date?: string;
  expiry_date?: string;
  is_confidential?: boolean;
  tags?: string;
}

// Company
export interface Company {
  id: string;
  name: string;
  registration_number: string | null;
  tax_number: string | null;
  epf_number: string | null;
  socso_code: string | null;
  eis_code: string | null;
  hrdf_number: string | null;
  address_line1: string | null;
  address_line2: string | null;
  city: string | null;
  state: string | null;
  postcode: string | null;
  country: string | null;
  phone: string | null;
  email: string | null;
  logo_url: string | null;
  hrdf_enabled: boolean | null;
  unpaid_leave_divisor: number | null;
  is_active: boolean | null;
  created_at: string;
  updated_at: string;
}

export interface UpdateCompanyRequest {
  name?: string;
  registration_number?: string;
  tax_number?: string;
  epf_number?: string;
  socso_code?: string;
  eis_code?: string;
  hrdf_number?: string;
  address_line1?: string;
  address_line2?: string;
  city?: string;
  state?: string;
  postcode?: string;
  country?: string;
  phone?: string;
  email?: string;
  logo_url?: string;
  hrdf_enabled?: boolean;
  unpaid_leave_divisor?: number;
}

export interface CompanyStats {
  total_employees: number;
  total_departments: number;
  total_payroll_groups: number;
  total_documents: number;
}

// Settings
export interface CompanySetting {
  id: string;
  company_id: string;
  category: string;
  key: string;
  value: unknown;
  label: string | null;
  description: string | null;
  updated_at: string;
  updated_by: string | null;
}

export interface SettingUpdate {
  category: string;
  key: string;
  value: unknown;
}

// Portal (Employee Self-Service)
export interface LeaveType {
  id: string;
  company_id: string;
  name: string;
  description: string | null;
  default_days: number;
  is_paid: boolean;
  is_active: boolean;
  max_carry_forward: number;
  carry_forward_expiry_months: number;
  is_system: boolean;
}

export interface LeaveBalance {
  id: string;
  leave_type_id: string;
  leave_type_name: string;
  is_paid: boolean;
  year: number;
  entitled_days: number;
  taken_days: number;
  pending_days: number;
  carried_forward: number;
}

export interface LeaveRequest {
  id: string;
  employee_id: string;
  company_id: string;
  leave_type_id: string;
  start_date: string;
  end_date: string;
  days: number;
  reason: string | null;
  status: 'pending' | 'approved' | 'rejected' | 'cancelled';
  reviewed_by: string | null;
  reviewed_at: string | null;
  review_notes: string | null;
  attachment_url: string | null;
  attachment_name: string | null;
  created_at: string;
  leave_type_name: string | null;
}

export interface CreateLeaveRequest {
  leave_type_id: string;
  start_date: string;
  end_date: string;
  days: number;
  reason?: string;
  attachment_url?: string;
  attachment_name?: string;
}

export interface UpdateLeaveRequest {
  employee_id?: string;
  leave_type_id?: string;
  start_date?: string;
  end_date?: string;
  days?: number;
  reason?: string;
  attachment_url?: string;
  attachment_name?: string;
}

export interface AdminCreateLeaveRequest extends CreateLeaveRequest {
  employee_id: string;
}

export interface Claim {
  id: string;
  employee_id: string;
  company_id: string;
  title: string;
  description: string | null;
  amount: number;
  category: string | null;
  receipt_url: string | null;
  receipt_file_name: string | null;
  expense_date: string;
  status: 'draft' | 'pending' | 'approved' | 'rejected' | 'processed' | 'cancelled';
  submitted_at: string | null;
  reviewed_by: string | null;
  reviewed_at: string | null;
  review_notes: string | null;
  created_at: string;
}

export interface CreateClaimRequest {
  title: string;
  description?: string;
  amount: number;
  category?: string;
  receipt_url?: string;
  receipt_file_name?: string;
  expense_date: string;
}

export interface UpdateClaimRequest {
  employee_id?: string;
  title?: string;
  description?: string;
  amount?: number;
  category?: string;
  receipt_url?: string;
  receipt_file_name?: string;
  expense_date?: string;
}

export interface AdminCreateClaimRequest extends CreateClaimRequest {
  employee_id: string;
}

export interface MyPayslip {
  id: string;
  payroll_run_id: string;
  period_year: number;
  period_month: number;
  period_start: string;
  period_end: string;
  pay_date: string;
  basic_salary: number;
  gross_salary: number;
  total_allowances: number;
  total_overtime: number;
  total_bonus: number;
  total_commission: number;
  total_claims: number;
  epf_employee: number;
  epf_employer: number;
  socso_employee: number;
  socso_employer: number;
  eis_employee: number;
  eis_employer: number;
  pcb_amount: number;
  zakat_amount: number;
  ptptn_amount: number;
  tabung_haji_amount: number;
  total_loan_deductions: number;
  total_other_deductions: number;
  unpaid_leave_deduction: number;
  total_deductions: number;
  net_salary: number;
  employer_cost: number;
  ytd_gross: number;
  ytd_epf_employee: number;
  ytd_pcb: number;
  ytd_socso_employee: number;
  ytd_eis_employee: number;
  ytd_zakat: number;
  ytd_net: number;
}

// Calendar
export interface Holiday {
  id: string;
  company_id: string;
  name: string;
  date: string;
  holiday_type: 'public_holiday' | 'company_holiday' | 'replacement_leave' | 'state_holiday';
  description: string | null;
  is_recurring: boolean;
  state: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateHolidayRequest {
  name: string;
  date: string;
  holiday_type?: string;
  description?: string;
  is_recurring?: boolean;
  state?: string;
}

export interface UpdateHolidayRequest {
  name?: string;
  date?: string;
  holiday_type?: string;
  description?: string;
  is_recurring?: boolean;
  state?: string;
}

export interface WorkingDayConfig {
  id: string;
  company_id: string;
  day_of_week: number;
  is_working_day: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpdateWorkingDaysRequest {
  days: { day_of_week: number; is_working_day: boolean }[];
}

export interface MonthCalendar {
  year: number;
  month: number;
  working_days: number;
  holidays: Holiday[];
  working_day_config: WorkingDayConfig[];
}

// Teams
export interface Team {
  id: string;
  company_id: string;
  name: string;
  description: string | null;
  tag: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
  created_by: string | null;
  updated_by: string | null;
}

export interface TeamWithCount extends Omit<Team, 'created_by' | 'updated_by'> {
  member_count: number | null;
}

export interface TeamMember {
  id: string;
  team_id: string;
  employee_id: string;
  role: string;
  joined_at: string;
  employee_name: string | null;
  employee_number: string | null;
  department: string | null;
  designation: string | null;
}

export interface CreateTeamRequest {
  name: string;
  description?: string;
  tag?: string;
}

export interface UpdateTeamRequest {
  name?: string;
  description?: string;
  tag?: string;
  is_active?: boolean;
}

export interface AddTeamMemberRequest {
  employee_id: string;
  role?: string;
}

// Overtime
export interface OvertimeApplication {
  id: string;
  employee_id: string;
  company_id: string;
  ot_date: string;
  start_time: string;
  end_time: string;
  hours: number;
  ot_type: 'normal' | 'rest_day' | 'public_holiday';
  reason: string | null;
  status: string;
  reviewed_by: string | null;
  reviewed_at: string | null;
  review_notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface OvertimeWithEmployee extends OvertimeApplication {
  employee_name: string | null;
  employee_number: string | null;
}

export interface CreateOvertimeRequest {
  ot_date: string;
  start_time: string;
  end_time: string;
  hours: number;
  ot_type?: 'normal' | 'rest_day' | 'public_holiday';
  reason?: string;
}

export interface UpdateOvertimeRequest {
  employee_id?: string;
  ot_date?: string;
  start_time?: string;
  end_time?: string;
  hours?: number;
  ot_type?: 'normal' | 'rest_day' | 'public_holiday';
  reason?: string;
}

export interface AdminCreateOvertimeRequest extends CreateOvertimeRequest {
  employee_id: string;
}

export interface TeamLeaveEntry {
  id: string;
  employee_id: string;
  employee_name: string;
  department: string | null;
  leave_type_name: string;
  start_date: string;
  end_date: string;
  days: number;
  status: string;
}

// Email / Letters
export type LetterType = 'welcome' | 'offer' | 'appointment' | 'warning' | 'termination' | 'promotion' | 'general';

export interface EmailTemplate {
  id: string;
  company_id: string;
  name: string;
  letter_type: LetterType;
  subject: string;
  body_html: string;
  is_active: boolean | null;
  created_at: string;
  updated_at: string;
}

export interface CreateEmailTemplateRequest {
  name: string;
  letter_type: LetterType;
  subject: string;
  body_html: string;
}

export interface UpdateEmailTemplateRequest {
  name?: string;
  subject?: string;
  body_html?: string;
  is_active?: boolean;
}

export interface EmailLog {
  id: string;
  company_id: string;
  employee_id: string | null;
  template_id: string | null;
  letter_type: string;
  recipient_email: string;
  recipient_name: string | null;
  subject: string;
  body_html: string;
  status: 'pending' | 'sent' | 'failed';
  error_message: string | null;
  sent_at: string | null;
  created_at: string;
}

export interface SendLetterRequest {
  employee_id?: string;
  recipient_email?: string;
  recipient_name?: string;
  letter_type: LetterType;
  subject: string;
  body_html: string;
  template_id?: string;
}

export interface PreviewLetterRequest {
  employee_id?: string;
  recipient_email?: string;
  recipient_name?: string;
  subject: string;
  body_html: string;
}

export interface PreviewLetterResponse {
  subject: string;
  body_html: string;
  recipient_email: string;
  recipient_name: string;
}

// ─── Employee Import ───

export interface FieldError {
  field: string;
  message: string;
}

export interface ImportRowValidation {
  row_number: number;
  status: 'valid' | 'error' | 'duplicate';
  errors: FieldError[];
  data: Record<string, string | null>;
}

export interface ImportValidationResponse {
  session_id: string;
  total_rows: number;
  valid_rows: number;
  error_rows: number;
  duplicate_rows: number;
  rows: ImportRowValidation[];
}

export interface ImportConfirmRequest {
  session_id: string;
  skip_invalid: boolean;
}

export interface ImportConfirmResponse {
  imported_count: number;
  skipped_count: number;
  errors: ImportRowValidation[];
}

// ─── Audit Trail ───

export interface AuditLog {
  id: string;
  company_id?: string;
  user_id: string | null;
  entity_type: string;
  entity_id: string | null;
  action: string;
  old_values: Record<string, unknown> | null;
  new_values: Record<string, unknown> | null;
  ip_address: string | null;
  user_agent?: string | null;
  description: string | null;
  created_at: string;
  user_email: string | null;
  user_full_name: string | null;
}

// ─── EA Form ───

export interface EaEmployeeSummary {
  employee_id: string;
  employee_name: string;
  employee_number: string;
  ic_number: string | null;
  ytd_gross: number;
  months_worked: number;
}

// ─── Backup / Data Migration ───

export interface ImportResult {
  new_company_id: string;
  new_company_name: string;
  is_overwrite: boolean;
  records_imported: Record<string, number>;
  warnings: string[];
}
