import { useEffect, useRef, useState } from 'react';
import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { Search, X } from 'lucide-react';
import { getEmployees } from '@/api/employees';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { useAuth } from '@/context/AuthContext';

/** The API clamps `per_page` to 100 (`handlers/employee.rs`). Ask for a page,
 *  not the roster: the server-side `search` is what finds EMP250, and a caller
 *  requesting 200 or 500 just gets the alphabetically-first 100 with no hint
 *  that anyone is missing. */
const PAGE_SIZE = 20;

/** Keystrokes are cheap; a request per keystroke is not. */
const SEARCH_DEBOUNCE_MS = 250;

interface EmployeePickerProps {
  value: string;
  onChange: (id: string, label: string) => void;
  placeholder?: string;
  /**
   * Label for an employee chosen elsewhere — an edit opening on a record whose
   * subject is nowhere in the first page of results. Without it the input reads
   * blank over a perfectly populated record, and one stray click reassigns it.
   */
  initialLabel?: string;
  /**
   * `true` (default) active staff only, `false` leavers only, `null` everyone.
   * Letters and documents legitimately address leavers; payroll and approvals
   * do not.
   */
  isActive?: boolean | null;
}

/**
 * Search-as-you-type employee selector. Every roster dropdown in the app used
 * to fetch `per_page: 200`/`500` and filter client-side, so on a tenant past
 * 100 employees the tail was simply unreachable — and silently so.
 */
export function EmployeePicker({
  value,
  onChange,
  placeholder = 'Search by name or employee number…',
  initialLabel,
  isActive = true,
}: EmployeePickerProps) {
  const { user } = useAuth();
  const companyId = user?.company_id ?? null;
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [open, setOpen] = useState(false);
  const [selectedLabel, setSelectedLabel] = useState(initialLabel ?? '');
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [search]);

  // `companyId` is in the key on purpose: after `PUT /auth/switch-company` a
  // bare key served the previous tenant's roster to the new one.
  const { data, isLoading } = useQuery({
    queryKey: ['employees-picker', companyId, isActive, debouncedSearch],
    queryFn: () => getEmployees({
      search: debouncedSearch || undefined,
      is_active: isActive ?? undefined,
      per_page: PAGE_SIZE,
    }),
    enabled: open,
    placeholderData: keepPreviousData,
  });
  const employees = data?.data ?? [];

  // Close on outside click so the list doesn't linger over the form.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open]);

  // Follow the controlled value when it is reset from outside (the filter-bar
  // "Clear" button, or the open-sessions tile). Otherwise the input keeps
  // showing the old employee while the table shows everyone — and the X button,
  // gated on `value`, disappears so the stale text cannot be cleared at all.
  // A label the user just picked wins over `initialLabel`; only a blank is seeded.
  useEffect(() => {
    if (!value) {
      setSelectedLabel('');
      return;
    }
    setSelectedLabel((previous) => previous || initialLabel || '');
  }, [value, initialLabel]);

  return (
    <div className="relative" ref={containerRef}>
      <div className="relative">
        <Search className="w-3.5 h-3.5 text-gray-400 absolute left-3 top-1/2 -translate-y-1/2" />
        <input
          type="text"
          value={open ? search : selectedLabel}
          onChange={e => { setSearch(e.target.value); setOpen(true); }}
          onFocus={() => { setSearch(''); setOpen(true); }}
          placeholder={placeholder}
          className="w-full pl-9 pr-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
        />
        {(value || selectedLabel) && !open && (
          <button
            type="button"
            onClick={() => { onChange('', ''); setSelectedLabel(''); }}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gray-400 hover:text-gray-700"
            title="Clear"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {open && (
        <div className="absolute z-10 mt-1 w-full bg-white border border-gray-200 rounded-lg shadow-lg max-h-56 overflow-y-auto">
          {isLoading ? (
            <div className="px-3 py-3 text-sm text-gray-400">Searching…</div>
          ) : employees.length === 0 ? (
            <div className="px-3 py-3 text-sm text-gray-400">No matching employees</div>
          ) : (
            employees.map(emp => (
              <button
                type="button"
                key={emp.id}
                onClick={() => {
                  const label = formatEmployeeLabel(emp.full_name, emp.employee_number) ?? emp.full_name;
                  onChange(emp.id, label);
                  setSelectedLabel(label);
                  setOpen(false);
                }}
                className="w-full text-left px-3 py-2 hover:bg-gray-50 transition-colors"
              >
                <div className="text-sm font-medium text-gray-900">{emp.full_name}</div>
                <div className="text-xs text-gray-400">
                  {emp.employee_number}{emp.department ? ` · ${emp.department}` : ''}
                </div>
              </button>
            ))
          )}
          {/* The `total` nobody was reading is what turns an invisible
              truncation into a visible one. */}
          {data && data.total > employees.length && (
            <div className="px-3 py-2 text-xs text-gray-400 border-t border-gray-100">
              Showing {employees.length} of {data.total} — keep typing to narrow
            </div>
          )}
        </div>
      )}
    </div>
  );
}
