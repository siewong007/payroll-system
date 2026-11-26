import type { UpdateEmployeeRequest } from '@/types';

/**
 * Fields the API classifies as payroll-sensitive — mirrors
 * `create_request_touches_payroll_fields` / `update_request_touches_payroll_fields`
 * in `backend/src/handlers/employee.rs`. Sending ANY of them without
 * `view_payroll` rejects the WHOLE request with 403, so a role holding only
 * `manage_employees` must never put them on the wire.
 *
 * Banking is on the list on purpose: it decides where salary lands. The read
 * side matches: `redact_payroll_fields` also strips banking, so a role without
 * `view_payroll` (e.g. hr_manager, manager) neither sees nor edits it. The
 * `clear_bank_*` flags are listed too — wiping banking is as sensitive as
 * overwriting it, and the server rejects them for the same roles.
 *
 * The `satisfies` clause is the only compile-time link back to the backend's
 * list: renaming or removing a field becomes a type error rather than a silent
 * 403 in production.
 */
export const PAYROLL_SENSITIVE_EMPLOYEE_FIELDS = [
  'basic_salary',
  'hourly_rate',
  'daily_rate',
  'bank_name',
  'bank_account_number',
  'bank_account_type',
  'clear_bank_name',
  'clear_bank_account_number',
  'tax_identification_number',
  'epf_number',
  'socso_number',
  'eis_number',
  'working_spouse',
  'epf_category',
  'clear_epf_category',
  'is_muslim',
  'zakat_eligible',
  'zakat_monthly_amount',
  'ptptn_monthly_amount',
  'tabung_haji_amount',
  'hrdf_contribution',
  'payroll_group_id',
  'salary_group',
] as const satisfies readonly (keyof UpdateEmployeeRequest)[];

/**
 * Drop every payroll-sensitive key from a form payload. The keys are *deleted*
 * rather than set to `undefined`: an explicit `null` on the wire still reads as
 * `is_some()` server-side and would trip the same 403.
 *
 * Omission is safe — `repositories/employees.rs` updates through
 * `COALESCE($n, column)`, so a field left out keeps its stored value and an
 * HR-only save can no longer clobber banking.
 */
export function stripPayrollFields<T extends Partial<UpdateEmployeeRequest>>(
  form: T,
): Partial<T> {
  const safe: Partial<T> = { ...form };
  for (const field of PAYROLL_SENSITIVE_EMPLOYEE_FIELDS) {
    delete safe[field as keyof T];
  }
  return safe;
}

/**
 * The one spelling of an employee's display label. A caller seeding
 * `EmployeePicker`'s `initialLabel` from a record it already holds must match
 * what the picker writes on selection, or the box changes shape the moment
 * anyone touches it.
 */
export function formatEmployeeLabel(name: string | null, number: string | null): string | undefined {
  if (!name) return undefined;
  return number ? `${name} (${number})` : name;
}
