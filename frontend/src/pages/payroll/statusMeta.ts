/// Payroll-run status display metadata, shared by the runs list, the detail
/// page and the overview. Lives outside the page modules because lazyNamed()
/// requires every export of a lazily imported module to be a component.
export const PAYROLL_STATUS_STYLES: Record<string, string> = {
  draft: 'bg-gray-100 text-gray-600',
  processing: 'bg-yellow-50 text-yellow-700',
  processed: 'bg-gray-100 text-gray-900',
  pending_approval: 'bg-blue-50 text-blue-700',
  approved: 'bg-green-50 text-green-700',
  paid: 'bg-emerald-50 text-emerald-700',
  cancelled: 'bg-red-50 text-red-700',
};

export const PAYROLL_STATUS_LABELS: Record<string, string> = {
  draft: 'Draft',
  processing: 'Processing',
  processed: 'Processed',
  pending_approval: 'Pending Approval',
  approved: 'Approved',
  paid: 'Paid',
  cancelled: 'Cancelled',
};
