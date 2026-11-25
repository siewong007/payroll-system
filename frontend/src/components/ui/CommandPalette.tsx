import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  BarChart3,
  Bell,
  Building2,
  Calculator,
  Calendar,
  CalendarDays,
  ClipboardCheck,
  Clock,
  DatabaseBackup,
  FileText,
  Globe,
  LayoutDashboard,
  Loader2,
  Mail,
  Moon,
  Plus,
  Receipt,
  ScanLine,
  ScrollText,
  Search,
  Settings,
  Shield,
  Sparkles,
  Sun,
  Upload,
  User,
  UserCheck,
  UserCog,
  Users,
  Users2,
  X,
} from 'lucide-react';
import { getEmployees } from '@/api/employees';
import { useAuth } from '@/context/AuthContext';
import { useAppearance } from '@/context/ThemeContext';
import { subscribeCommandPalette } from '@/lib/commandPalette';
import { canUseAdminConsole, hasAnyRole, hasOnlyEmployeeRole, type AppRole } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
import type { PermissionKey } from '@/api/permissions';

interface PaletteItem {
  id: string;
  group: 'employees' | 'pages' | 'actions' | 'preferences';
  label: string;
  sublabel?: string;
  icon: typeof Search;
  keywords?: string;
  onSelect: () => void;
}

export function CommandPalette() {
  const { t, i18n } = useTranslation();
  const { user, isAuthenticated } = useAuth();
  const { resolvedTheme, toggleTheme } = useAppearance();
  const navigate = useNavigate();

  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const wasOpenRef = useRef(false);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery('');
    setDebouncedQuery('');
    setActiveIndex(0);
  }, []);

  useEffect(() => {
    return subscribeCommandPalette(() => {
      previousFocusRef.current = document.activeElement as HTMLElement | null;
      setOpen(true);
      setQuery('');
      setDebouncedQuery('');
      setActiveIndex(0);
    });
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (!open || trimmed.length < 2) {
      setDebouncedQuery('');
      return;
    }
    const timer = window.setTimeout(() => setDebouncedQuery(trimmed), 200);
    return () => window.clearTimeout(timer);
  }, [open, query]);

  useEffect(() => {
    if (!isAuthenticated) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen((prev) => {
          if (!prev) {
            previousFocusRef.current = document.activeElement as HTMLElement | null;
          }
          return !prev;
        });
      } else if (e.key === 'Escape' && open) {
        e.preventDefault();
        e.stopImmediatePropagation();
        close();
      } else if (e.key === 'Tab' && open && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'input:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex="-1"])',
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [close, isAuthenticated, open]);

  useEffect(() => {
    if (open) {
      wasOpenRef.current = true;
      requestAnimationFrame(() => inputRef.current?.focus());
    } else if (wasOpenRef.current) {
      wasOpenRef.current = false;
      requestAnimationFrame(() => previousFocusRef.current?.focus?.());
    }
  }, [open]);

  const canSearchEmployees =
    Boolean(user) &&
    !hasOnlyEmployeeRole(user) &&
    userCan(user, 'view_employees') &&
    !hasAnyRole(user, ['super_admin']);

  const { data: employeeHits, isFetching: searchingEmployees } = useQuery({
    queryKey: ['palette-employees', debouncedQuery],
    queryFn: () => getEmployees({ search: debouncedQuery, page: 1, per_page: 5 }),
    enabled: open && canSearchEmployees && debouncedQuery.length >= 2,
    staleTime: 30_000,
  });

  const staticItems = useMemo<PaletteItem[]>(() => {
    if (!user) return [];
    const list: PaletteItem[] = [];
    const isPortalOnly = hasOnlyEmployeeRole(user);

    const addRoute = (
      id: string,
      label: string,
      href: string,
      icon: typeof Search,
      opts?: { requires?: PermissionKey; hideFor?: AppRole[]; requiresEmployee?: boolean },
    ) => {
      if (opts?.hideFor && hasAnyRole(user, opts.hideFor)) return;
      if (opts?.requires && !userCan(user, opts.requires)) return;
      if (opts?.requiresEmployee && !user.employee_id) return;
      list.push({
        id,
        group: 'pages',
        label,
        icon,
        keywords: href,
        onSelect: () => {
          navigate(href);
          close();
        },
      });
    };

    if (!isPortalOnly) {
      addRoute('dashboard', t('nav.dashboard'), '/dashboard', LayoutDashboard, { hideFor: ['super_admin'] });
      addRoute('company', t('nav.company'), '/company', Building2, { hideFor: ['super_admin'] });
      addRoute('employees', t('nav.employees'), '/employees', Users, { requires: 'view_employees', hideFor: ['super_admin'] });
      addRoute('payroll', t('nav.payroll'), '/payroll', Calculator, { requires: 'view_payroll' });
      addRoute('teams', t('nav.teams'), '/teams', Users2, { requires: 'view_teams', hideFor: ['super_admin'] });
      addRoute('calendar', t('nav.calendar'), '/calendar', CalendarDays, { requires: 'view_calendar', hideFor: ['super_admin'] });
      addRoute('attendance', t('nav.attendance'), '/attendance', ScanLine, { requires: 'view_attendance', hideFor: ['super_admin'] });
      addRoute('approvals', t('nav.approvals'), '/approvals', ClipboardCheck, { requires: 'view_approvals', hideFor: ['super_admin'] });
      addRoute('reports', t('nav.reports'), '/reports', BarChart3, { requires: 'view_reports' });
      addRoute('documents', t('nav.documents'), '/documents', FileText, { requires: 'view_documents', hideFor: ['super_admin'] });
      addRoute('letters', t('nav.letters'), '/letters', Mail, { requires: 'view_email_logs', hideFor: ['super_admin'] });
      addRoute('settings', t('nav.settings'), '/settings', Settings, { requires: 'manage_company_settings', hideFor: ['super_admin'] });
      addRoute('my-attendance', t('nav.myAttendance'), '/my/attendance', UserCheck, { requiresEmployee: true });
      addRoute('companies', t('nav.companies'), '/companies', Building2, { requires: 'manage_companies' });
      addRoute('users', t('nav.users'), '/users', UserCog, { requires: 'manage_users' });
      addRoute('roles', t('nav.roles'), '/roles', Shield, { requires: 'view_user_directory' });
      addRoute('user-groups', t('nav.userGroups'), '/user-groups', Users2, { requires: 'manage_users' });
      addRoute('attendance-settings', t('nav.attendanceSettings'), '/admin/attendance-settings', ScanLine, { requires: 'manage_platform_settings' });
      addRoute('audit-trail', t('nav.auditTrail'), '/audit-trail', ScrollText, { requires: 'view_audit_log' });
      addRoute('backup', t('nav.backup'), '/backup', DatabaseBackup, { requires: 'manage_backups' });
      addRoute('account-security', t('nav.accountSecurity'), '/account', Shield);
    }

    if (user.employee_id || isPortalOnly) {
      addRoute('portal-profile', t('nav.portal.profile'), '/portal/profile', User);
      addRoute('portal-attendance', t('nav.portal.attendance'), '/portal/attendance', ScanLine);
      addRoute('portal-payslips', t('nav.portal.payslips'), '/portal/payslips', FileText);
      addRoute('portal-leave', t('nav.portal.leave'), '/portal/leave', Calendar);
      addRoute('portal-claims', t('nav.portal.claims'), '/portal/claims', Receipt);
      addRoute('portal-overtime', t('nav.portal.overtime'), '/portal/overtime', Clock);
      addRoute('portal-team-calendar', t('nav.portal.teamCalendar'), '/portal/team-calendar', Users);
      addRoute('portal-notifications', t('nav.portal.notifications'), '/portal/notifications', Bell);
    }

    if (user.employee_id || isPortalOnly) {
      list.push(
        {
          id: 'act-portal-checkin',
          group: 'actions',
          label: t('ux.commandPalette.portalCheckIn'),
          icon: ScanLine,
          onSelect: () => {
            navigate(isPortalOnly ? '/portal/attendance' : '/my/attendance');
            close();
          },
        },
        {
          id: 'act-portal-leave',
          group: 'actions',
          label: t('ux.commandPalette.portalLeave'),
          icon: Calendar,
          onSelect: () => {
            navigate('/portal/leave');
            close();
          },
        },
        {
          id: 'act-portal-claims',
          group: 'actions',
          label: t('ux.commandPalette.portalClaims'),
          icon: Receipt,
          onSelect: () => {
            navigate('/portal/claims');
            close();
          },
        },
      );
    }

    if (canUseAdminConsole(user) && isPortalOnly === false) {
      if (userCan(user, 'manage_employees') && !hasAnyRole(user, ['super_admin'])) {
        list.push({
          id: 'act-new-emp',
          group: 'actions',
          label: t('ux.commandPalette.newEmployee'),
          icon: Plus,
          onSelect: () => {
            navigate('/employees/new');
            close();
          },
        });
      }
      if (userCan(user, 'import_employees') && !hasAnyRole(user, ['super_admin'])) {
        list.push({
          id: 'act-import-emp',
          group: 'actions',
          label: t('ux.commandPalette.importEmployees'),
          icon: Upload,
          onSelect: () => {
            navigate('/employees/import');
            close();
          },
        });
      }
      if (userCan(user, 'manage_payroll_draft')) {
        list.push({
          id: 'act-process-payroll',
          group: 'actions',
          label: t('ux.commandPalette.processPayroll'),
          icon: Sparkles,
          onSelect: () => {
            navigate('/payroll/process');
            close();
          },
        });
      }
    }

    list.push({
      id: 'pref-theme',
      group: 'preferences',
      label: resolvedTheme === 'dark' ? t('ux.commandPalette.switchToLight') : t('ux.commandPalette.switchToDark'),
      icon: resolvedTheme === 'dark' ? Sun : Moon,
      onSelect: () => {
        toggleTheme();
        close();
      },
    });

    (['en', 'ms', 'zh-CN', 'zh-TW'] as const).forEach((lng) => {
      if (i18n.language !== lng) {
        list.push({
          id: `pref-lang-${lng}`,
          group: 'preferences',
          label: `${t('language.label')}: ${t(`language.${lng}`)}`,
          icon: Globe,
          keywords: lng,
          onSelect: () => {
            void i18n.changeLanguage(lng);
            close();
          },
        });
      }
    });

    return list;
  }, [close, i18n, navigate, resolvedTheme, t, toggleTheme, user]);

  const employeeItems = useMemo<PaletteItem[]>(() => {
    if (!debouncedQuery || !employeeHits?.data) return [];
    return employeeHits.data.map((emp) => ({
      id: `emp-${emp.id}`,
      group: 'employees' as const,
      label: emp.full_name,
      sublabel: [emp.employee_number, emp.department].filter(Boolean).join(' · '),
      icon: User,
      onSelect: () => {
        navigate(`/employees/${emp.id}`);
        close();
      },
    }));
  }, [close, debouncedQuery, employeeHits, navigate]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const matchedStatic = !q
      ? staticItems
      : staticItems.filter(
          (item) =>
            item.label.toLowerCase().includes(q) ||
            (item.keywords && item.keywords.toLowerCase().includes(q)),
        );
    return [...employeeItems, ...matchedStatic];
  }, [employeeItems, query, staticItems]);

  const clampedIndex = Math.min(activeIndex, Math.max(0, filtered.length - 1));

  useEffect(() => {
    if (!open) return;
    itemRefs.current[clampedIndex]?.scrollIntoView?.({ block: 'nearest' });
  }, [clampedIndex, open]);

  const handleListKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((prev) => (filtered.length === 0 ? 0 : (prev + 1) % filtered.length));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((prev) => (filtered.length === 0 ? 0 : (prev - 1 + filtered.length) % filtered.length));
    } else if (e.key === 'Enter' && filtered[clampedIndex]) {
      e.preventDefault();
      filtered[clampedIndex].onSelect();
    }
  };

  const groups: { key: PaletteItem['group']; title: string }[] = [
    { key: 'employees', title: t('ux.employeesSearch.groupTitle') },
    { key: 'pages', title: t('ux.commandPalette.groupPages') },
    { key: 'actions', title: t('ux.commandPalette.groupActions') },
    { key: 'preferences', title: t('ux.commandPalette.groupPreferences') },
  ];

  if (!open) return null;

  return (
    <>
      <div
        onClick={close}
        className="animate-fade-in fixed inset-0 z-[75] bg-slate-950/55 backdrop-blur-sm"
        aria-hidden="true"
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t('nav.quickSearch')}
        className="animate-scale-in fixed inset-x-4 top-[12vh] z-[76] mx-auto max-w-xl overflow-hidden rounded-2xl bg-white dark:bg-slate-900 shadow-2xl ring-1 ring-black/10 dark:ring-white/10"
      >
            <div className="flex items-center gap-3 border-b border-gray-200/80 dark:border-slate-800 px-4 py-3.5">
              {searchingEmployees ? (
                <Loader2 className="h-4 w-4 shrink-0 animate-spin text-[var(--accent-1)]" aria-hidden="true" />
              ) : (
                <Search className="h-4 w-4 shrink-0 text-gray-400" aria-hidden="true" />
              )}
              <input
                ref={inputRef}
                type="text"
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActiveIndex(0);
                }}
                onKeyDown={handleListKeyDown}
                placeholder={t('ux.commandPalette.placeholder')}
                aria-label={t('ux.commandPalette.placeholder')}
                className="flex-1 bg-transparent text-base sm:text-sm text-gray-900 dark:text-slate-100 placeholder:text-gray-400 focus:outline-none"
              />
              <button
                type="button"
                onClick={close}
                aria-label={t('a11y.closeDialog')}
                className="sm:hidden min-h-10 min-w-10 -mr-2 inline-flex items-center justify-center rounded-lg text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
              >
                <X className="h-4 w-4" />
              </button>
              <kbd className="hidden sm:inline-flex items-center rounded-md border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] font-semibold text-gray-500 dark:text-slate-400">
                ESC
              </kbd>
            </div>

            <div className="max-h-[55vh] overflow-y-auto p-2 scrollbar-thin">
              {filtered.length === 0 ? (
                <p className="py-10 text-center text-sm text-gray-400 dark:text-slate-500">
                  {searchingEmployees ? t('ux.employeesSearch.searching') : t('ux.commandPalette.noMatches')}
                </p>
              ) : (
                groups.map((group) => {
                  const groupItems = filtered.filter((i) => i.group === group.key);
                  if (groupItems.length === 0) return null;
                  return (
                    <div key={group.key} className="mb-2 last:mb-0">
                      <p className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-widest text-gray-400 dark:text-slate-500">
                        {group.title}
                      </p>
                      <div className="space-y-0.5">
                        {groupItems.map((item) => {
                          const flatIndex = filtered.indexOf(item);
                          const isSelected = flatIndex === clampedIndex;
                          const Icon = item.icon;
                          return (
                            <button
                              key={item.id}
                              ref={(el) => {
                                itemRefs.current[flatIndex] = el;
                              }}
                              type="button"
                              onMouseEnter={() => setActiveIndex(flatIndex)}
                              onClick={item.onSelect}
                              className={`flex w-full items-center justify-between gap-3 rounded-xl px-3 py-2.5 text-left text-sm transition-colors ${
                                isSelected
                                  ? 'bg-[var(--accent-soft)] font-semibold text-[var(--accent-1)] dark:text-white'
                                  : 'text-gray-700 hover:bg-gray-100/70 dark:text-slate-300 dark:hover:bg-slate-800/60'
                              }`}
                            >
                              <span className="flex items-center gap-3 min-w-0">
                                <Icon className="h-4 w-4 shrink-0 opacity-80" aria-hidden="true" />
                                <span className="truncate">{item.label}</span>
                              </span>
                              {item.sublabel && (
                                <span className="shrink-0 text-xs font-normal text-gray-400 dark:text-slate-500">
                                  {item.sublabel}
                                </span>
                              )}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            <div className="hidden sm:flex items-center justify-between border-t border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-950/50 px-4 py-2 text-[11px] text-gray-400 dark:text-slate-500">
              <span>↑↓ {t('ux.commandPalette.hintNavigate')}</span>
              <span>↵ {t('ux.commandPalette.hintOpen')}</span>
              <span>ESC {t('ux.commandPalette.hintClose')}</span>
            </div>
      </div>
    </>
  );
}
