import { type KeyboardEvent, useId, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useToast } from '@/lib/toast';
import { useConfirm } from '@/lib/confirm';
import { useQuery, useMutation, useQueryClient, keepPreviousData } from '@tanstack/react-query';
import {
  Clock,
  Filter,
  Loader2,
  Plus,
  MapPin,
  AlertCircle,
  Calendar,
  LogIn,
  LogOut,
  MoreVertical,
  ChevronLeft,
  ChevronRight,
  Pencil,
  AlertTriangle,
  Timer,
  Download,
  ListChecks,
  Settings,
  Link2,
  WifiOff,
} from 'lucide-react';
import {
  getAttendanceMethod,
  getAttendanceRecords,
  downloadAttendanceCsv,
  runAbsentMarking,
  setCompanyAttendanceMethod,
  type AttendanceRecordWithEmployee,
} from '@/api/attendance';
import { usePermissions } from '@/lib/usePermissions';
import { zonedDateKey } from '@/lib/attendance';
import { getErrorMessage } from '@/lib/utils';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { WorkScheduleCard } from '@/components/attendance/WorkScheduleCard';
import { GeofenceCard } from '@/components/attendance/GeofenceCard';
import { NetworkCard } from '@/components/attendance/NetworkCard';
import { KioskCredentialsModal } from './KioskCredentialsModal';
import {
  METHOD_CONFIG,
  STATUS_COLORS,
  attMethodLabel,
  attStatusLabel,
  formatDate,
  formatTime,
  todayInZone,
} from './attendanceHelpers';
import { QrPanel } from './QrPanel';
import { StatsBar, SummaryTab } from './AttendanceSummaryTab';
import { EditAttendanceModal, ManualEntryModal, RecordCard } from './AttendanceRecordModals';

// ─── Main Page ────────────────────────────────────────────────────────────────

/** Empty filter set. Named so "clear" and the triage shortcuts can't drift from
 *  the initial state as filters are added. */
const NO_FILTERS = {
  date_from: '', date_to: '', status: '', method: '', employee_id: '',
  open_only: false, outside_geofence_only: false, offsite_network_only: false,
};

export function AttendancePage() {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const [searchParams, setSearchParams] = useSearchParams();
  const { can } = usePermissions();
  const queryClient = useQueryClient();

  const urlStatus = searchParams.get('status') ?? '';
  const urlOpenOnly = searchParams.get('open_only') === '1';
  const urlOutsideGeofence = searchParams.get('outside_geofence') === '1';
  const urlOffsiteNetwork = searchParams.get('offsite_network') === '1';
  const urlTab = searchParams.get('tab');

  const canView = can('view_attendance');
  const canEdit = can('manage_attendance');
  const canGenerateQr = can('generate_attendance_qr');
  const canManageKiosks = can('manage_kiosks');
  const canManageSchedule = can('manage_work_schedules');
  const canManageGeofence = can('manage_geofence');
  const canManageNetworks = can('manage_attendance_networks');
  const canConfigure = canManageSchedule || canManageGeofence || canManageNetworks;

  const initialTab: 'records' | 'summary' | 'settings' =
    urlTab === 'settings' && canConfigure
      ? 'settings'
      : urlTab === 'summary'
        ? 'summary'
        : 'records';

  const [tab, setTab] = useState<'records' | 'summary' | 'settings'>(initialTab);
  const [filterOverrides, setFilterOverrides] = useState<typeof NO_FILTERS | null>(null);
  const [page, setPage] = useState(1);
  const tabsIdPrefix = useId();
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const filters = useMemo<typeof NO_FILTERS>(
    () =>
      filterOverrides ?? {
        ...NO_FILTERS,
        status: urlStatus,
        open_only: urlOpenOnly,
        outside_geofence_only: urlOutsideGeofence,
        offsite_network_only: urlOffsiteNetwork,
      },
    [filterOverrides, urlStatus, urlOpenOnly, urlOutsideGeofence, urlOffsiteNetwork],
  );

  const [showManual, setShowManual] = useState(false);
  const [showHeaderKiosks, setShowHeaderKiosks] = useState(false);
  const [editRecord, setEditRecord] = useState<AttendanceRecordWithEmployee | null>(null);
  const [exporting, setExporting] = useState(false);
  const [pageError, setPageError] = useState('');
  const [backfillDate, setBackfillDate] = useState('');
  const perPage = 50;

  const { data: method } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
    enabled: canView || canConfigure || canManageKiosks || canGenerateQr,
  });

  const timeZone = method?.timezone || 'Asia/Kuala_Lumpur';
  const today = todayInZone(timeZone);

  const canSwitchMethod =
    Boolean(method?.allow_company_override) && can('set_company_attendance_method');

  const { data: result, isLoading, isFetching, isError, error, refetch } = useQuery({
    queryKey: ['attendance-records', filters, page],
    queryFn: () => getAttendanceRecords({
      date_from:   filters.date_from   || undefined,
      date_to:     filters.date_to     || undefined,
      status:      filters.status      || undefined,
      method:      filters.method      || undefined,
      employee_id: filters.employee_id || undefined,
      open_only:   filters.open_only   || undefined,
      outside_geofence_only: filters.outside_geofence_only || undefined,
      offsite_network_only: filters.offsite_network_only || undefined,
      page,
      per_page:  perPage,
    }),
    enabled: canView,
    placeholderData: keepPreviousData,
  });

  const records = result?.data ?? [];
  const totalPages = result?.total_pages ?? 1;
  const total = result?.total ?? 0;
  const forbidden = !canView || (error as { response?: { status?: number } } | null)?.response?.status === 403;

  const methodMut = useMutation({
    mutationFn: (m: string | null) => setCompanyAttendanceMethod(m),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      setPageError('');
    },
    onError: (e) => setPageError(getErrorMessage(e, t('attendance.page.methodFailed'))),
  });

  const backfillMut = useMutation({
    mutationFn: (date: string) => runAbsentMarking(date),
    onSuccess: (res) => {
      void queryClient.invalidateQueries({ queryKey: ['attendance-records'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-summary'] });
      setPageError('');
      setBackfillDate('');
      toast.success(t('attendance.page.backfillDone', { count: res.marked, date: res.date }));
    },
    onError: (e) => setPageError(getErrorMessage(e, t('attendance.page.backfillFailed'))),
  });

  // Reset to page 1 when filters change
  const updateFilter = (key: keyof typeof NO_FILTERS, value: string | boolean) => {
    setFilterOverrides((prev) => ({ ...(prev ?? filters), [key]: value }));
    setPage(1);
  };

  const clearFilters = () => {
    setFilterOverrides(NO_FILTERS);
    if (urlStatus || urlOpenOnly || urlOutsideGeofence || urlOffsiteNetwork) {
      const next = new URLSearchParams(searchParams);
      next.delete('status');
      next.delete('open_only');
      next.delete('outside_geofence');
      next.delete('offsite_network');
      setSearchParams(next, { replace: true });
    }
    setPage(1);
  };

  const hasFilters = Boolean(
    filters.date_from || filters.date_to || filters.status ||
    filters.method || filters.employee_id || filters.open_only ||
    filters.outside_geofence_only || filters.offsite_network_only
  );

  const showOpenSessions = () => {
    setTab('records');
    setFilterOverrides({ ...NO_FILTERS, open_only: true });
    setPage(1);
  };

  const handleBackfill = async () => {
    if (!backfillDate) return;
    const ok = await confirmDialog({
      description: t('attendance.page.backfillConfirm', { date: backfillDate }),
      confirmLabel: t('attendance.page.markAbsences'),
      variant: 'warning',
    });
    if (ok) {
      backfillMut.mutate(backfillDate);
    }
  };

  const tabItems = [
    ...(canView
      ? [
          { key: 'records' as const, label: t('attendance.page.tabs.records'), icon: Clock },
          { key: 'summary' as const, label: t('attendance.page.tabs.summary'), icon: ListChecks },
        ]
      : [{ key: 'records' as const, label: t('attendance.page.tabs.records'), icon: Clock }]),
    ...(canConfigure
      ? [{ key: 'settings' as const, label: t('attendance.page.tabs.settings'), icon: Settings }]
      : []),
  ];

  const showQrPanel = method?.method === 'qr_code' && (canGenerateQr || canManageKiosks);

  const handleTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, currentIndex: number) => {
    let targetIndex: number | null = null;
    if (event.key === 'ArrowRight') {
      targetIndex = (currentIndex + 1) % tabItems.length;
    } else if (event.key === 'ArrowLeft') {
      targetIndex = (currentIndex - 1 + tabItems.length) % tabItems.length;
    } else if (event.key === 'Home') {
      targetIndex = 0;
    } else if (event.key === 'End') {
      targetIndex = tabItems.length - 1;
    }
    if (targetIndex === null) return;
    event.preventDefault();
    const target = tabItems[targetIndex];
    if (!target) return;
    setTab(target.key);
    tabRefs.current[targetIndex]?.focus();
  };

  const activeTabId = `${tabsIdPrefix}-tab-${tab}`;
  const activePanelId = `${tabsIdPrefix}-panel-${tab}`;

  return (
    <div className="space-y-6">
      {/* Header */}
      <PageHeader
        className="mb-0"
        title={t('attendance.page.title')}
        actions={
          <div className="flex flex-wrap gap-2 shrink-0">
            {canManageKiosks && method?.method !== 'qr_code' && (
              <button
                type="button"
                onClick={() => setShowHeaderKiosks(true)}
                className="btn-secondary flex-1 sm:flex-none min-h-11 sm:min-h-10"
              >
                <Link2 className="w-4 h-4 shrink-0" aria-hidden="true" />
                {t('attendance.kioskModal.title')}
              </button>
            )}
            {canView && tab === 'records' && (
              <button
                type="button"
                onClick={async () => {
                  setExporting(true);
                  setPageError('');
                  try {
                    await downloadAttendanceCsv({
                      date_from:   filters.date_from   || undefined,
                      date_to:     filters.date_to     || undefined,
                      status:      filters.status      || undefined,
                      method:      filters.method      || undefined,
                      employee_id: filters.employee_id || undefined,
                      open_only:             filters.open_only || undefined,
                      outside_geofence_only: filters.outside_geofence_only || undefined,
                      offsite_network_only:  filters.offsite_network_only || undefined,
                    });
                  } catch (e) {
                    setPageError(getErrorMessage(e, t('attendance.page.exportFailed')));
                  } finally {
                    setExporting(false);
                  }
                }}
                disabled={exporting}
                className="btn-secondary flex-1 sm:flex-none min-h-11 sm:min-h-10"
              >
                {exporting ? (
                  <Loader2 className="w-4 h-4 shrink-0 animate-spin" aria-hidden="true" />
                ) : (
                  <Download className="w-4 h-4 shrink-0" aria-hidden="true" />
                )}
                {exporting ? t('common.exporting') : t('attendance.page.exportCsv')}
              </button>
            )}
            {canEdit && (
              <button
                type="button"
                onClick={() => setShowManual(true)}
                className="btn-primary flex-1 sm:flex-none min-h-11 sm:min-h-10"
              >
                <Plus className="w-4 h-4 shrink-0" aria-hidden="true" />
                {t('attendance.page.manualEntry')}
              </button>
            )}
          </div>
        }
      />

      {method && (
        <div className="-mt-3 text-sm text-gray-500 dark:text-slate-400 flex flex-wrap items-center gap-2">
          <span>{t('attendance.page.method')}</span>
          {canSwitchMethod ? (
            <select
              value={method.is_company_override ? method.method : ''}
              onChange={e => methodMut.mutate(e.target.value || null)}
              disabled={methodMut.isPending}
              aria-label={t('attendance.page.method')}
              className="form-input !w-auto min-h-11 sm:min-h-9 !py-1.5 !px-3 sm:text-sm font-medium disabled:opacity-50"
            >
              <option value="">
                {method.is_company_override
                  ? t('attendance.page.platformDefault')
                  : `${attMethodLabel(method.method)} (${t('attendance.page.methodDefault')})`}
              </option>
              <option value="qr_code">{attMethodLabel('qr_code')}</option>
              <option value="face_id">{attMethodLabel('face_id')}</option>
            </select>
          ) : (
            <span className="font-medium text-gray-700 dark:text-slate-200">
              {attMethodLabel(method.method)}
            </span>
          )}
          {method.is_company_override && (
            <span className="text-xs bg-amber-100 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300 px-2 py-0.5 rounded-full font-medium">
              {t('attendance.page.companyOverride')}
            </span>
          )}
        </div>
      )}

      {pageError && (
        <div role="alert" className="px-4 py-3 rounded-xl bg-red-50 dark:bg-red-500/15 border border-red-200 dark:border-red-500/30 text-sm text-red-700 dark:text-red-300 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" aria-hidden="true" />
          {pageError}
        </div>
      )}

      {canView && <StatsBar today={today} onShowOpen={showOpenSessions} />}

      {/* Tabs */}
      <div role="tablist" aria-label={t('attendance.page.title')} className="flex items-center gap-1 border-b border-gray-200 dark:border-slate-800 overflow-x-auto">
        {tabItems.map((item, idx) => {
          const selected = tab === item.key;
          return (
            <button
              key={item.key}
              ref={(el) => {
                tabRefs.current[idx] = el;
              }}
              id={`${tabsIdPrefix}-tab-${item.key}`}
              type="button"
              role="tab"
              aria-selected={selected}
              tabIndex={selected ? 0 : -1}
              aria-controls={selected ? activePanelId : undefined}
              onClick={() => setTab(item.key)}
              onKeyDown={(e) => handleTabKeyDown(e, idx)}
              className={`min-h-11 flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium border-b-2 -mb-px transition-colors whitespace-nowrap shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                selected
                  ? 'border-[var(--accent-1)] text-gray-900 dark:text-slate-100'
                  : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
              }`}
            >
              <item.icon className="w-4 h-4" aria-hidden="true" />
              {item.label}
            </button>
          );
        })}
      </div>

      <div id={activePanelId} role="tabpanel" aria-labelledby={activeTabId}>
      {tab === 'settings' && canConfigure ? (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {canManageSchedule && <WorkScheduleCard />}
          {canManageGeofence && <GeofenceCard />}
          {canManageNetworks && (
            <div className="lg:col-span-2">
              <NetworkCard enabled={canManageNetworks} />
            </div>
          )}
        </div>
      ) : tab === 'summary' && canView ? (
        <SummaryTab today={today} canExport={canView} canView={canView} />
      ) : (
      <div className={`grid grid-cols-1 gap-6 ${showQrPanel ? '2xl:grid-cols-[300px_minmax(0,1fr)]' : ''}`}>
        {/* QR Panel — only in QR mode */}
        {showQrPanel && (
          <div>
            <QrPanel canGenerate={canGenerateQr} canManageKiosks={canManageKiosks} />
          </div>
        )}

        {/* Records Table */}
        <div className="min-w-0">
          <div className="card !p-0 overflow-hidden">
            {/* Filters */}
            <div className="flex flex-wrap items-end gap-2.5 sm:gap-3 p-4 sm:p-5 border-b border-gray-100 dark:border-slate-800">
              <div className="flex w-full sm:w-auto items-center gap-2 self-center text-sm text-gray-600 dark:text-slate-300">
                <Filter className="w-4 h-4" aria-hidden="true" /> {t('attendance.page.filters')}
              </div>
              <label className="flex flex-col gap-1 min-w-[8.5rem] flex-1 sm:flex-none">
                <span className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('attendance.page.from')}</span>
                <input
                  type="date"
                  value={filters.date_from}
                  max={filters.date_to || undefined}
                  onChange={e => updateFilter('date_from', e.target.value)}
                  aria-label={t('attendance.page.from')}
                  className="form-input !w-full min-h-11 sm:min-h-9 !py-1.5 sm:text-xs"
                />
              </label>
              <label className="flex flex-col gap-1 min-w-[8.5rem] flex-1 sm:flex-none">
                <span className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('attendance.page.to')}</span>
                <input
                  type="date"
                  value={filters.date_to}
                  min={filters.date_from || undefined}
                  onChange={e => updateFilter('date_to', e.target.value)}
                  aria-label={t('attendance.page.to')}
                  className="form-input !w-full min-h-11 sm:min-h-9 !py-1.5 sm:text-xs"
                />
              </label>
              <select
                value={filters.status}
                onChange={e => updateFilter('status', e.target.value)}
                aria-label={t('attendance.page.allStatus')}
                className="form-input !w-auto min-w-[8.5rem] min-h-11 sm:min-h-9 flex-1 sm:flex-none !py-1.5 sm:text-xs"
              >
                <option value="">{t('attendance.page.allStatus')}</option>
                <option value="present">{attStatusLabel('present')}</option>
                <option value="late">{attStatusLabel('late')}</option>
                <option value="absent">{attStatusLabel('absent')}</option>
                <option value="half_day">{attStatusLabel('half_day')}</option>
              </select>
              <select
                value={filters.method}
                onChange={e => updateFilter('method', e.target.value)}
                aria-label={t('attendance.page.allMethods')}
                className="form-input !w-auto min-w-[8.5rem] min-h-11 sm:min-h-9 flex-1 sm:flex-none !py-1.5 sm:text-xs"
              >
                <option value="">{t('attendance.page.allMethods')}</option>
                <option value="qr_code">{attMethodLabel('qr_code')}</option>
                <option value="face_id">{attMethodLabel('face_id')}</option>
                <option value="manual">{attMethodLabel('manual')}</option>
              </select>
              <label className="min-h-11 sm:min-h-9 flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600 dark:text-slate-300 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={filters.open_only}
                  onChange={e => updateFilter('open_only', e.target.checked)}
                  className="w-4 h-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)]"
                />
                {t('attendance.page.openOnly')}
              </label>
              <label className="min-h-11 sm:min-h-9 flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600 dark:text-slate-300 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={filters.outside_geofence_only}
                  onChange={e => updateFilter('outside_geofence_only', e.target.checked)}
                  className="w-4 h-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)]"
                />
                {t('attendance.page.offsiteOnly')}
              </label>
              <label className="min-h-11 sm:min-h-9 flex w-full sm:w-auto items-center gap-2 text-sm text-gray-600 dark:text-slate-300 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={filters.offsite_network_only}
                  onChange={e => updateFilter('offsite_network_only', e.target.checked)}
                  className="w-4 h-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)]"
                />
                {t('attendance.page.offsiteNetworkOnly')}
              </label>
              {hasFilters && (
                <button
                  type="button"
                  onClick={clearFilters}
                  className="min-h-11 sm:min-h-9 px-2 text-xs text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-100 underline rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  {t('common.clear')}
                </button>
              )}
              <div className="w-full sm:w-72">
                <EmployeePicker
                  value={filters.employee_id}
                  onChange={(id) => updateFilter('employee_id', id)}
                  placeholder={t('attendance.page.filterEmployee')}
                />
              </div>
            </div>

            {/* Backfill */}
            {canEdit && (
              <div className="flex flex-wrap items-center gap-2 px-4 sm:px-5 py-3 border-b border-gray-100 dark:border-slate-800 bg-gray-50/60 dark:bg-slate-950/40">
                <span className="text-xs text-gray-500 dark:text-slate-400">
                  {t('attendance.page.backfillHint')}
                </span>
                <input
                  type="date"
                  value={backfillDate}
                  max={today}
                  onChange={e => setBackfillDate(e.target.value)}
                  aria-label={t('attendance.page.markAbsences')}
                  className="form-input !w-auto min-h-11 sm:min-h-9 !py-1.5 !px-2.5 sm:text-xs"
                />
                <button
                  type="button"
                  onClick={() => void handleBackfill()}
                  disabled={!backfillDate || backfillMut.isPending}
                  className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 text-xs"
                >
                  {backfillMut.isPending && <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />}
                  {backfillMut.isPending ? t('common.running') : t('attendance.page.markAbsences')}
                </button>
                <span className="text-xs text-gray-500 dark:text-slate-400">
                  {t('attendance.page.backfillSafe')}
                </span>
              </div>
            )}

            {/* Table */}
            {isLoading ? (
              <div
                role="status"
                aria-busy="true"
                aria-label={t('common.loading')}
                className="p-4 sm:p-5 space-y-3"
              >
                {[0, 1, 2, 3].map((i) => (
                  <Skeleton key={i} className="h-14 w-full rounded-xl" />
                ))}
              </div>
            ) : isError || !canView ? (
              <EmptyState
                icon={AlertCircle}
                title={forbidden ? t('attendance.page.forbidden') : t('attendance.page.loadFailed')}
                action={
                  !forbidden ? (
                    <button type="button" onClick={() => refetch()} className="btn-primary">
                      {t('common.retry')}
                    </button>
                  ) : undefined
                }
              />
            ) : records.length === 0 ? (
              <EmptyState
                icon={Calendar}
                title={
                  filters.open_only
                    ? t('attendance.page.emptyOpen')
                    : filters.outside_geofence_only
                      ? t('attendance.page.emptyOffsite')
                      : filters.offsite_network_only
                        ? t('attendance.page.emptyOffnetwork')
                        : t('attendance.page.empty')
                }
                action={
                  hasFilters ? (
                    <button type="button" onClick={clearFilters} className="btn-secondary">
                      {t('ux.table.clearFilters')}
                    </button>
                  ) : undefined
                }
              />
            ) : (
              <div className={isFetching && !isLoading ? 'opacity-75 transition-opacity' : undefined}>
                <div className="divide-y divide-gray-100 dark:divide-slate-800 lg:hidden">
                  {records.map(r => (
                    <RecordCard
                      key={r.id}
                      record={r}
                      canEdit={canEdit}
                      timeZone={timeZone}
                      onEdit={() => setEditRecord(r)}
                    />
                  ))}
                </div>

                <div className="hidden lg:block overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-gray-100 dark:border-slate-800">
                        {[
                          t('common.date'),
                          t('common.employee'),
                          t('attendance.records.checkIn'),
                          t('attendance.records.checkOut'),
                          t('attendance.summary.hours'),
                          t('attendance.records.method'),
                          t('common.status'),
                          t('attendance.records.location'),
                          ...(canEdit ? [''] : []),
                        ].map((h, idx) => (
                          <th key={`${h}-${idx}`} className="px-5 py-3 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-gray-100 dark:divide-slate-800/80">
                      {records.map(r => {
                        const statusCfg = STATUS_COLORS[r.status as keyof typeof STATUS_COLORS];
                        const methodCfg = METHOD_CONFIG[r.method as keyof typeof METHOD_CONFIG];
                        const MethodIcon = methodCfg?.icon ?? MoreVertical;
                        const isAbsentPlaceholder = r.status === 'absent';
                        const overnightOut =
                          !isAbsentPlaceholder &&
                          Boolean(r.check_out_at) &&
                          zonedDateKey(r.check_out_at as string, timeZone) > zonedDateKey(r.check_in_at, timeZone);

                        return (
                          <tr key={r.id} className="hover:bg-gray-50/70 dark:hover:bg-slate-800/40 transition-colors">
                            <td className="px-5 py-3.5 whitespace-nowrap text-gray-600 dark:text-slate-300 tabular-nums">
                              {formatDate(r.check_in_at, timeZone)}
                            </td>
                            <td className="px-5 py-3.5">
                              <div className="font-medium text-gray-900 dark:text-slate-100">{r.full_name}</div>
                              <div className="text-xs text-gray-500 dark:text-slate-400">{r.employee_number}</div>
                              {r.notes && (
                                <div className="text-xs text-gray-500 dark:text-slate-400 truncate max-w-[220px]" title={r.notes}>
                                  {r.notes}
                                </div>
                              )}
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700 dark:text-slate-200 whitespace-nowrap">
                              <div className="flex items-center gap-1.5">
                                <LogIn className="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400" aria-hidden="true" />
                                {isAbsentPlaceholder ? '—' : formatTime(r.check_in_at, timeZone)}
                              </div>
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700 dark:text-slate-200 whitespace-nowrap">
                              <div className="flex items-center gap-1.5">
                                <LogOut className="w-3.5 h-3.5 text-gray-500 dark:text-slate-400" aria-hidden="true" />
                                {isAbsentPlaceholder ? '—' : formatTime(r.check_out_at, timeZone)}
                                {overnightOut && (
                                  <span className="rounded bg-gray-100 dark:bg-slate-800 px-1 py-px text-[10px] font-medium text-gray-600 dark:text-slate-300">
                                    {t('attendance.records.nextDay')}
                                  </span>
                                )}
                              </div>
                            </td>
                            <td className="px-5 py-3.5 tabular-nums text-gray-700 dark:text-slate-200 whitespace-nowrap">
                              {r.hours_worked != null ? (
                                <div className="text-xs">
                                  <span>{t('attendance.hoursShort', { value: Number(r.hours_worked).toFixed(1) })}</span>
                                  {r.overtime_hours != null && Number(r.overtime_hours) > 0 && (
                                    <span className="ml-1 text-amber-600 dark:text-amber-400 font-medium">
                                      <Timer className="w-3 h-3 inline" aria-hidden="true" /> {t('attendance.hoursOtShort', { value: Number(r.overtime_hours).toFixed(1) })}
                                    </span>
                                  )}
                                </div>
                              ) : (
                                <span className="text-gray-400 dark:text-slate-500 text-xs">—</span>
                              )}
                            </td>
                            <td className="px-5 py-3.5 whitespace-nowrap">
                              <div className={`flex items-center gap-1.5 ${methodCfg?.color ?? 'text-gray-600 dark:text-slate-300'}`}>
                                <MethodIcon className="w-3.5 h-3.5" aria-hidden="true" />
                                <span className="text-xs font-medium">{methodCfg ? attMethodLabel(r.method) : r.method}</span>
                              </div>
                            </td>
                            <td className="px-5 py-3.5 whitespace-nowrap">
                              <div className="flex items-center gap-1.5">
                                {statusCfg && (
                                  <span className={`px-2.5 py-0.5 rounded-full text-xs font-medium ${statusCfg}`}>
                                    {attStatusLabel(r.status)}
                                  </span>
                                )}
                                {r.is_outside_geofence && (
                                  <span title={t('attendance.records.outsideGeofence')} className="text-amber-500 dark:text-amber-400">
                                    <AlertTriangle className="w-3.5 h-3.5" aria-label={t('attendance.records.outsideGeofence')} />
                                  </span>
                                )}
                                {r.is_offsite_network && (
                                  <span title={t('attendance.records.offsiteNetwork')} className="text-amber-600 dark:text-amber-400">
                                    <WifiOff className="w-3.5 h-3.5" aria-label={t('attendance.records.offsiteNetwork')} />
                                  </span>
                                )}
                              </div>
                            </td>
                            <td className="px-5 py-3.5 whitespace-nowrap">
                              {(r.latitude && r.longitude) || (r.checkout_latitude && r.checkout_longitude) ? (
                                <div className="flex flex-col gap-1 text-xs">
                                  {r.latitude && r.longitude && (
                                    <a
                                      href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
                                      target="_blank"
                                      rel="noopener noreferrer"
                                      className="flex items-center gap-1 text-sky-600 dark:text-sky-400 hover:text-sky-700 dark:hover:text-sky-300 hover:underline"
                                    >
                                      <MapPin className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                                      {t('attendance.records.checkInLocation')}
                                    </a>
                                  )}
                                  {r.checkout_latitude && r.checkout_longitude && (
                                    <a
                                      href={`https://maps.google.com/?q=${r.checkout_latitude},${r.checkout_longitude}`}
                                      target="_blank"
                                      rel="noopener noreferrer"
                                      className="flex items-center gap-1 text-teal-600 dark:text-teal-400 hover:text-teal-700 dark:hover:text-teal-300 hover:underline"
                                    >
                                      <MapPin className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
                                      {t('attendance.records.checkOutLocation')}
                                    </a>
                                  )}
                                </div>
                              ) : (
                                <span className="text-gray-400 dark:text-slate-500 text-xs">—</span>
                              )}
                            </td>
                            {canEdit && (
                              <td className="px-3 py-3.5 text-right">
                                <button
                                  type="button"
                                  onClick={() => setEditRecord(r)}
                                  className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                                  title={t('attendance.records.editRecord')}
                                  aria-label={`${t('attendance.records.editRecord')} \u2014 ${r.full_name}`}
                                >
                                  <Pencil className="w-3.5 h-3.5" aria-hidden="true" />
                                </button>
                              </td>
                            )}
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                {/* Pagination */}
                {totalPages > 1 && (
                  <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 px-4 sm:px-5 py-3 border-t border-gray-100 dark:border-slate-800">
                    <p className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                      {t('common.showing', { from: (page - 1) * perPage + 1, to: Math.min(page * perPage, total), total })}
                    </p>
                    <div className="flex items-center justify-center gap-1">
                      <button
                        type="button"
                        onClick={() => setPage(p => Math.max(1, p - 1))}
                        disabled={page <= 1}
                        aria-label={t('common.previousPage')}
                        className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                      >
                        <ChevronLeft className="w-4 h-4" aria-hidden="true" />
                      </button>
                      {Array.from({ length: Math.min(totalPages, 5) }, (_, i) => {
                        let p: number;
                        if (totalPages <= 5) p = i + 1;
                        else if (page <= 3) p = i + 1;
                        else if (page >= totalPages - 2) p = totalPages - 4 + i;
                        else p = page - 2 + i;
                        return (
                          <button
                            key={p}
                            type="button"
                            onClick={() => setPage(p)}
                            aria-current={p === page ? 'page' : undefined}
                            className={`min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 rounded-lg text-xs font-medium tabular-nums transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                              p === page
                                ? 'bg-[var(--accent-1)] text-white'
                                : 'hover:bg-gray-100 dark:hover:bg-slate-800 text-gray-600 dark:text-slate-300'
                            }`}
                          >
                            {p}
                          </button>
                        );
                      })}
                      <button
                        type="button"
                        onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                        disabled={page >= totalPages}
                        aria-label={t('common.nextPage')}
                        className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-lg text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                      >
                        <ChevronRight className="w-4 h-4" aria-hidden="true" />
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
      )}
      </div>

      {showManual && <ManualEntryModal onClose={() => setShowManual(false)} />}
      {showHeaderKiosks && <KioskCredentialsModal onClose={() => setShowHeaderKiosks(false)} />}
      {editRecord && <EditAttendanceModal record={editRecord} onClose={() => setEditRecord(null)} />}
    </div>
  );
}
