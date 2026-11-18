import type { TFunction } from 'i18next';
import type { LeaveReportRow } from '@/api/reports';

export function getLeaveEligibility(
  t: TFunction,
  leaveType: string,
  gender: string | null,
  maritalStatus: string | null,
  numChildren: number | null,
): { eligible: boolean; reason?: string } {
  const name = leaveType.toLowerCase();

  if (name.includes('maternity')) {
    if (gender !== 'female') return { eligible: false, reason: t('reports.eligibility.femaleOnly') };
  }

  if (name.includes('paternity')) {
    if (gender !== 'male') return { eligible: false, reason: t('reports.eligibility.maleOnly') };
    if ((numChildren ?? 0) === 0) {
      return { eligible: false, reason: t('reports.eligibility.noChildren') };
    }
  }

  if (name.includes('marriage')) {
    if (maritalStatus === 'married') {
      return { eligible: false, reason: t('reports.eligibility.alreadyMarried') };
    }
    if (!maritalStatus || maritalStatus === 'single') return { eligible: true };
  }

  return { eligible: true };
}

export interface EmployeeLeaveGroup {
  employee_name: string;
  employee_number: string;
  department: string | null;
  gender: string | null;
  marital_status: string | null;
  num_children: number | null;
  leaves: LeaveReportRow[];
}

export function groupByEmployee(rows: LeaveReportRow[]): EmployeeLeaveGroup[] {
  const map = new Map<string, EmployeeLeaveGroup>();
  for (const r of rows) {
    let group = map.get(r.employee_number);
    if (!group) {
      group = {
        employee_name: r.employee_name,
        employee_number: r.employee_number,
        department: r.department,
        gender: r.gender,
        marital_status: r.marital_status,
        num_children: r.num_children,
        leaves: [],
      };
      map.set(r.employee_number, group);
    }
    group.leaves.push(r);
  }
  return Array.from(map.values());
}
