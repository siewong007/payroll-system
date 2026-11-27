import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { Search, X } from 'lucide-react';
import { getEmployees } from '@/api/employees';
import { formatEmployeeLabel } from '@/lib/employeeFields';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage } from '@/lib/utils';

/** The API clamps `per_page` to 100 (`handlers/employee.rs`). Ask for a page,
 *  not the roster: the server-side `search` is what finds EMP250, and a caller
 *  requesting 200 or 500 just gets the alphabetically-first 100 with no hint
 *  that anyone is missing. */
const PAGE_SIZE = 20;

/** Keystrokes are cheap; a request per keystroke is not. */
const SEARCH_DEBOUNCE_MS = 250;

interface EmployeePickerProps {
  id?: string;
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
  disabled?: boolean;
  'aria-describedby'?: string;
  'aria-invalid'?: boolean;
}

/**
 * Search-as-you-type employee selector. Every roster dropdown in the app used
 * to fetch `per_page: 200`/`500` and filter client-side, so on a tenant past
 * 100 employees the tail was simply unreachable — and silently so.
 */
export function EmployeePicker({
  id,
  value,
  onChange,
  placeholder,
  initialLabel,
  isActive = true,
  disabled = false,
  'aria-describedby': ariaDescribedBy,
  'aria-invalid': ariaInvalid,
}: EmployeePickerProps) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const companyId = user?.company_id ?? null;
  const listboxId = useId();
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [selectedLabel, setSelectedLabel] = useState(initialLabel ?? '');
  const containerRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const lastSelectedRef = useRef<{ id: string; label: string }>({
    id: value,
    label: initialLabel ?? '',
  });

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [search]);

  // `companyId` is in the key on purpose: after `PUT /auth/switch-company` a
  // bare key served the previous tenant's roster to the new one.
  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['employees-picker', companyId, isActive, debouncedSearch],
    queryFn: () => getEmployees({
      search: debouncedSearch || undefined,
      is_active: isActive ?? undefined,
      per_page: PAGE_SIZE,
    }),
    enabled: open && !disabled,
    placeholderData: keepPreviousData,
  });
  const employees = data?.data ?? [];
  const safeActiveIndex = open && employees.length > 0
    ? Math.min(activeIndex, employees.length - 1)
    : -1;
  const activeOptionId = safeActiveIndex >= 0 && employees[safeActiveIndex]
    ? `${listboxId}-opt-${safeActiveIndex}`
    : undefined;

  useEffect(() => {
    if (safeActiveIndex >= 0) {
      optionRefs.current[safeActiveIndex]?.scrollIntoView?.({ block: 'nearest' });
    }
  }, [safeActiveIndex]);

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
      lastSelectedRef.current = { id: '', label: '' };
      setSelectedLabel('');
      return;
    }
    if (lastSelectedRef.current.id !== value) {
      const nextLabel = initialLabel ?? '';
      lastSelectedRef.current = { id: value, label: nextLabel };
      setSelectedLabel(nextLabel);
      return;
    }
    setSelectedLabel((previous) => previous || initialLabel || '');
  }, [value, initialLabel]);

  const handleSelect = (emp: { id: string; full_name: string; employee_number: string }) => {
    const label = formatEmployeeLabel(emp.full_name, emp.employee_number) ?? emp.full_name;
    lastSelectedRef.current = { id: emp.id, label };
    onChange(emp.id, label);
    setSelectedLabel(label);
    setOpen(false);
    setActiveIndex(-1);
  };

  const showClear = !disabled && Boolean(value || selectedLabel) && !open;

  return (
    <div className="relative" ref={containerRef}>
      <div className="relative">
        <Search className="w-3.5 h-3.5 text-[var(--subtle-foreground)] absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
        <input
          id={id}
          type="text"
          role="combobox"
          aria-expanded={open}
          aria-haspopup="listbox"
          aria-autocomplete="list"
          aria-controls={open ? listboxId : undefined}
          aria-activedescendant={activeOptionId}
          aria-describedby={ariaDescribedBy}
          aria-invalid={ariaInvalid}
          disabled={disabled}
          value={open ? search : selectedLabel}
          onChange={(e) => {
            setSearch(e.target.value);
            setOpen(true);
            setActiveIndex(-1);
          }}
          onFocus={() => {
            if (disabled) return;
            setSearch('');
            setOpen(true);
            setActiveIndex(-1);
          }}
          onClick={() => {
            if (disabled || open) return;
            setSearch('');
            setOpen(true);
            setActiveIndex(-1);
          }}
          onKeyDown={(e) => {
            if (disabled) return;
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              if (!open) {
                setOpen(true);
                setActiveIndex(0);
              } else if (employees.length > 0) {
                setActiveIndex((prev) => (prev + 1 < employees.length ? prev + 1 : 0));
              }
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              if (open && employees.length > 0) {
                setActiveIndex((prev) => (prev > 0 ? prev - 1 : employees.length - 1));
              }
            } else if (e.key === 'Enter' && open && safeActiveIndex >= 0 && employees[safeActiveIndex]) {
              e.preventDefault();
              handleSelect(employees[safeActiveIndex]);
            } else if (e.key === 'Escape' && open) {
              e.preventDefault();
              setOpen(false);
              setActiveIndex(-1);
            } else if (e.key === 'Tab' && open) {
              setOpen(false);
              setActiveIndex(-1);
            }
          }}
          placeholder={placeholder ?? t('employees.pickerPlaceholder')}
          className={`form-input !pl-9 ${showClear ? '!pr-10' : '!pr-3'} disabled:opacity-60 disabled:cursor-not-allowed`}
        />
        {showClear && (
          <button
            type="button"
            onClick={() => {
              onChange('', '');
              setSelectedLabel('');
              setActiveIndex(-1);
            }}
            className="absolute right-0.5 top-1/2 -translate-y-1/2 min-h-9 min-w-9 flex items-center justify-center rounded-md text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors"
            aria-label={t('common.clear')}
            title={t('common.clear')}
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      {open && (
        <div
          id={listboxId}
          role="listbox"
          className="absolute z-20 mt-1 w-full bg-[var(--card)] border border-[var(--border)] rounded-lg shadow-md max-h-56 overflow-y-auto"
        >
          {isLoading ? (
            <div className="px-3 py-2.5 text-[13px] text-[var(--muted-foreground)]">{t('employees.searching')}</div>
          ) : isError ? (
            <div className="px-3 py-2.5 text-[13px] text-red-600 dark:text-red-400 flex items-center justify-between gap-2">
              <span className="truncate">{getErrorMessage(error, t('employees.searchFailed'))}</span>
              <button
                type="button"
                onClick={() => refetch()}
                className="text-xs font-medium underline shrink-0 hover:text-red-800 dark:hover:text-red-300"
              >
                {t('common.retry')}
              </button>
            </div>
          ) : employees.length === 0 ? (
            <div className="px-3 py-2.5 text-[13px] text-[var(--muted-foreground)]">{t('employees.noMatch')}</div>
          ) : (
            employees.map((emp, idx) => {
              const isSelected = value === emp.id;
              const isActiveOption = idx === safeActiveIndex;
              return (
                <button
                  ref={(el) => {
                    optionRefs.current[idx] = el;
                  }}
                  id={`${listboxId}-opt-${idx}`}
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  key={emp.id}
                  onMouseEnter={() => setActiveIndex(idx)}
                  onClick={() => handleSelect(emp)}
                  className={`w-full text-left px-3 py-2 transition-colors ${
                    isActiveOption
                      ? 'bg-[var(--surface-subtle)]'
                      : isSelected
                      ? 'bg-[var(--surface-subtle)]/70'
                      : 'hover:bg-[var(--surface-subtle)]/60'
                  }`}
                >
                  <div className="text-[13px] font-medium text-[var(--foreground)] truncate">{emp.full_name}</div>
                  <div className="text-xs text-[var(--muted-foreground)] truncate">
                    {emp.employee_number}{emp.department ? ` · ${emp.department}` : ''}
                  </div>
                </button>
              );
            })
          )}
          {/* The `total` nobody was reading is what turns an invisible
              truncation into a visible one. */}
          {data && data.total > employees.length && (
            <div className="px-3 py-1.5 text-xs text-[var(--subtle-foreground)] border-t border-[var(--border)]">
              {t('employees.showingPartial', { shown: employees.length, total: data.total })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
