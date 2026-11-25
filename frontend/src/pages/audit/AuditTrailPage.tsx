import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { AlertTriangle, ScrollText, X, ChevronLeft, ChevronRight } from 'lucide-react';
import { getAuditFilterOptions, getAuditLogs } from '@/api/audit';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { formatDateTime } from '@/lib/format';
import { getErrorMessage } from '@/lib/utils';
import type { AuditLog } from '@/types';

/**
 * Fallback label for a value the filter response has not supplied — a row
 * rendered before the options query resolves, or an entity type whose rows
 * exist on another page of results. Mirrors the backend's derivation closely
 * enough to read the same; the backend's version is authoritative.
 */
const formatEntityType = (entityType: string) => {
  const words = entityType.replace(/_/g, ' ').trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : entityType;
};


export function AuditTrailPage() {
  const { t } = useTranslation();
  const [entityType, setEntityType] = useState('');
  // One record's full history. Set by clicking a row's entity id rather than by
  // typing a UUID — the point is to answer "who changed *this*, and why".
  const [entityId, setEntityId] = useState('');
  const [action, setAction] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [page, setPage] = useState(1);
  const [selectedLog, setSelectedLog] = useState<AuditLog | null>(null);
  const perPage = 25;

  // The filter vocabulary comes from the company's own rows, so the dropdowns
  // offer exactly what exists. Cached longer than the log list: the set of
  // entity types a company writes changes when the product does, not when
  // someone clicks. A failure here leaves the dropdowns empty rather than
  // breaking the page — the log list is the point, the filters are a
  // refinement.
  const { data: filterOptions } = useQuery({
    queryKey: ['audit-logs', 'filters'],
    queryFn: getAuditFilterOptions,
    staleTime: 5 * 60_000,
  });
  const entityTypeOptions = useMemo(() => filterOptions?.entity_types ?? [], [filterOptions]);
  const actionOptions = useMemo(() => filterOptions?.actions ?? [], [filterOptions]);
  const actionLabels = useMemo(
    () => new Map(actionOptions.map((option) => [option.value, option.label])),
    [actionOptions],
  );
  const entityLabels = useMemo(
    () => new Map(entityTypeOptions.map((option) => [option.value, option.label])),
    [entityTypeOptions],
  );

  const {
    data,
    isLoading,
    isError,
    error: queryError,
    refetch,
  } = useQuery({
    queryKey: ['audit-logs', entityType, entityId, action, startDate, endDate, page],
    queryFn: () => getAuditLogs({
      entity_type: entityType || undefined,
      entity_id: entityId || undefined,
      action: action || undefined,
      start_date: startDate || undefined,
      end_date: endDate || undefined,
      page,
      per_page: perPage,
    }),
    placeholderData: keepPreviousData,
  });

  const logs = data?.data ?? [];
  const total = data?.total ?? 0;
  const totalPages = Math.ceil(total / perPage);

  const clearFilters = () => {
    setEntityType('');
    setEntityId('');
    setAction('');
    setStartDate('');
    setEndDate('');
    setPage(1);
  };

  const hasFilters = Boolean(entityType || entityId || action || startDate || endDate);

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('audit.title')}
        subtitle={t('audit.subtitle')}
        actions={
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-slate-400">
            <ScrollText className="w-4 h-4" aria-hidden="true" />
            <span>{t('audit.recordsCount', { count: total })}</span>
          </div>
        }
      />

      {/* Filters */}
      <div className="card p-4">
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
          <div>
            <label htmlFor="audit-entity-type" className="block text-xs font-medium text-gray-500 dark:text-slate-400 mb-1">
              {t('audit.entityType')}
            </label>
            <select
              id="audit-entity-type"
              value={entityType}
              onChange={(e) => { setEntityType(e.target.value); setPage(1); }}
              className="form-input"
            >
              <option value="">{t('audit.allTypes')}</option>
              {entityTypeOptions.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="audit-action" className="block text-xs font-medium text-gray-500 dark:text-slate-400 mb-1">
              {t('audit.action')}
            </label>
            <select
              id="audit-action"
              value={action}
              onChange={(e) => { setAction(e.target.value); setPage(1); }}
              className="form-input"
            >
              <option value="">{t('audit.allActions')}</option>
              {actionOptions.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="audit-start-date" className="block text-xs font-medium text-gray-500 dark:text-slate-400 mb-1">
              {t('common.from')}
            </label>
            <input
              id="audit-start-date"
              type="date"
              value={startDate}
              onChange={(e) => { setStartDate(e.target.value); setPage(1); }}
              className="form-input"
            />
          </div>
          <div>
            <label htmlFor="audit-end-date" className="block text-xs font-medium text-gray-500 dark:text-slate-400 mb-1">
              {t('common.to')}
            </label>
            <input
              id="audit-end-date"
              type="date"
              value={endDate}
              onChange={(e) => { setEndDate(e.target.value); setPage(1); }}
              className="form-input"
            />
          </div>
        </div>
        {(hasFilters || entityId) && (
          <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2 text-xs text-gray-600 dark:text-slate-300">
              {/* The record scope has no control of its own — surface it so it can't
                  silently narrow the trail. */}
              {entityId && (
                <span className="inline-flex items-center gap-1.5 pl-2.5 pr-1 py-1 bg-gray-100 dark:bg-slate-800 rounded-full">
                  <span className="font-medium">{t('audit.record')}</span>
                  <span className="font-mono">{entityId.slice(0, 8)}...</span>
                  <button
                    type="button"
                    onClick={() => { setEntityId(''); setPage(1); }}
                    aria-label={t('audit.clearRecordFilter')}
                    className="min-h-8 min-w-8 sm:min-h-5 sm:min-w-5 inline-flex items-center justify-center rounded-full text-gray-400 dark:text-slate-500 hover:text-gray-900 dark:hover:text-slate-100 hover:bg-gray-200 dark:hover:bg-slate-700"
                  >
                    <X className="w-3 h-3" aria-hidden="true" />
                  </button>
                </span>
              )}
            </div>
            {hasFilters && (
              <button
                type="button"
                onClick={clearFilters}
                className="min-h-10 sm:min-h-8 px-3 py-1.5 text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg inline-flex items-center gap-1.5 transition-colors"
              >
                <X className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.clear')}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Table */}
      <div className="card p-0 overflow-hidden">
        {isLoading ? (
          <div className="flex items-center justify-center py-20">
            <div className="spinner" />
          </div>
        ) : isError ? (
          <EmptyState
            icon={AlertTriangle}
            title={t('audit.loadFailed')}
            description={getErrorMessage(queryError, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        ) : logs.length === 0 ? (
          <EmptyState
            icon={ScrollText}
            title={t('audit.noLogs')}
            action={
              hasFilters ? (
                <button type="button" onClick={clearFilters} className="btn-secondary">
                  {t('common.clear')}
                </button>
              ) : undefined
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100 dark:border-slate-800 bg-gray-50/50 dark:bg-slate-800/50">
                  <th className="text-left px-4 py-3 font-medium text-gray-500 dark:text-slate-400">{t('audit.timestamp')}</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500 dark:text-slate-400">{t('audit.user')}</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500 dark:text-slate-400">{t('audit.entity')}</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500 dark:text-slate-400">{t('audit.action')}</th>
                  <th className="text-left px-4 py-3 font-medium text-gray-500 dark:text-slate-400">{t('audit.details')}</th>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <tr
                    key={log.id}
                    tabIndex={0}
                    onClick={() => setSelectedLog(log)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        setSelectedLog(log);
                      }
                    }}
                    className="border-b border-gray-50 dark:border-slate-800/70 hover:bg-gray-50/50 dark:hover:bg-slate-800/40 cursor-pointer transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--ring)]"
                  >
                    <td className="px-4 py-3 text-gray-600 dark:text-slate-300 whitespace-nowrap">
                      {formatDateTime(log.created_at)}
                    </td>
                    <td className="px-4 py-3">
                      <div className="text-gray-900 dark:text-slate-100">{log.user_full_name || '-'}</div>
                      <div className="text-xs text-gray-400 dark:text-slate-500">{log.user_email}</div>
                    </td>
                    <td className="px-4 py-3">
                      <span className="inline-flex px-2 py-0.5 text-xs font-medium bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-300 rounded-full">
                        {entityLabels.get(log.entity_type) ?? formatEntityType(log.entity_type)}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <ActionBadge action={log.action} label={actionLabels.get(log.action)} />
                    </td>
                    <td className="px-4 py-3 text-gray-500 dark:text-slate-400 text-xs max-w-[260px]">
                      <div className="truncate">{log.description || '-'}</div>
                      {log.entity_id && (
                        <button
                          type="button"
                          // stopPropagation: the row opens the detail drawer, and
                          // filtering to the record is a different intent.
                          onClick={(e) => {
                            e.stopPropagation();
                            setEntityId(log.entity_id!);
                            setPage(1);
                          }}
                          title={t('audit.showRecordHistory')}
                          className="mt-0.5 font-mono text-[11px] text-gray-400 dark:text-slate-500 hover:text-gray-900 dark:hover:text-slate-200 hover:underline"
                        >
                          {log.entity_id.slice(0, 8)}...
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3 border-t border-gray-100 dark:border-slate-800">
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('common.pageOfTotal', { page, totalPages, total })}
            </p>
            <div className="flex gap-1">
              <button
                type="button"
                onClick={() => setPage(p => Math.max(1, p - 1))}
                disabled={page <= 1}
                aria-label={t('common.previousPage')}
                className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronLeft className="w-4 h-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                disabled={page >= totalPages}
                aria-label={t('common.nextPage')}
                className="min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronRight className="w-4 h-4" aria-hidden="true" />
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Detail Modal */}
      {selectedLog && (
        <AuditDetailModal
          log={selectedLog}
          entityLabel={entityLabels.get(selectedLog.entity_type)}
          actionLabel={actionLabels.get(selectedLog.action)}
          onClose={() => setSelectedLog(null)}
        />
      )}
    </div>
  );
}

/**
 * Colour is keyed on the action's leading verb, not the whole string.
 *
 * The backend writes three conventions for the same verbs — `create`,
 * `create_employee`, `create_claim_admin` — so an exact-match palette coloured
 * 9 of the 31 real actions and left the rest grey. Matching the verb covers all
 * three shapes with one entry each.
 *
 * Deliberately a partial map over `string` with a total default rather than an
 * exhaustive `Record` over a union of known actions: the action vocabulary
 * lives in the database, TypeScript cannot check a runtime list, and a union
 * hand-written here would be one more copy with nothing to catch it drifting.
 * An unrecognised verb renders grey, which is visibly plain rather than broken.
 */
const ACTION_VERB_COLORS: Record<string, string> = {
  create: 'bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300',
  update: 'bg-blue-50 text-blue-700 dark:bg-blue-500/15 dark:text-blue-300',
  delete: 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300',
  revoke: 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300',
  approve: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300',
  submit: 'bg-blue-50 text-blue-700 dark:bg-blue-500/15 dark:text-blue-300',
  return: 'bg-amber-50 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300',
  lock: 'bg-gray-900 text-white dark:bg-slate-200 dark:text-slate-900',
  reject: 'bg-orange-50 text-orange-700 dark:bg-orange-500/15 dark:text-orange-300',
  cancel: 'bg-yellow-50 text-yellow-700 dark:bg-yellow-500/15 dark:text-yellow-300',
  process: 'bg-indigo-50 text-indigo-700 dark:bg-indigo-500/15 dark:text-indigo-300',
  send: 'bg-purple-50 text-purple-700 dark:bg-purple-500/15 dark:text-purple-300',
  bulk: 'bg-teal-50 text-teal-700 dark:bg-teal-500/15 dark:text-teal-300',
};

function actionBadgeClass(action: string): string {
  const verb = action.split('_')[0];
  return ACTION_VERB_COLORS[verb] ?? 'bg-gray-50 text-gray-700 dark:bg-slate-800 dark:text-slate-300';
}

function ActionBadge({ action, label }: { action: string; label?: string }) {
  return (
    <span
      className={`inline-flex px-2 py-0.5 text-xs font-medium rounded-full ${actionBadgeClass(action)}`}
    >
      {label ?? action}
    </span>
  );
}

function formatAuditValue(val: unknown): string {
  if (val === undefined) return '—';
  if (val === null) return 'null';
  if (typeof val === 'string') return val === '' ? '""' : val;
  if (typeof val === 'number' || typeof val === 'boolean') return String(val);
  return JSON.stringify(val);
}

function AuditDetailModal({
  log,
  entityLabel,
  actionLabel,
  onClose,
}: {
  log: AuditLog;
  entityLabel?: string;
  actionLabel?: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const hasOld = Boolean(log.old_values && Object.keys(log.old_values).length > 0);
  const hasNew = Boolean(log.new_values && Object.keys(log.new_values).length > 0);
  const diffRows = useMemo(() => {
    if (!hasOld && !hasNew) return [];
    const oldObj = log.old_values ?? {};
    const newObj = log.new_values ?? {};
    const keys = Array.from(new Set([...Object.keys(oldObj), ...Object.keys(newObj)])).sort();
    return keys.map((key) => {
      const oldPresent = Object.prototype.hasOwnProperty.call(oldObj, key);
      const newPresent = Object.prototype.hasOwnProperty.call(newObj, key);
      const oldVal = oldPresent ? oldObj[key] : undefined;
      const newVal = newPresent ? newObj[key] : undefined;
      const changed =
        oldPresent !== newPresent || JSON.stringify(oldVal) !== JSON.stringify(newVal);
      return { key, oldVal, newVal, changed };
    });
  }, [hasOld, hasNew, log.old_values, log.new_values]);

  return (
    <Modal open onClose={onClose} title={t('audit.detailTitle')} maxWidth="max-w-2xl">
      <div className="space-y-4">
        <p className="text-xs text-gray-400 dark:text-slate-500 -mt-2">{formatDateTime(log.created_at)}</p>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
          <div>
            <p className="text-gray-500 dark:text-slate-400">{t('audit.user')}</p>
            <p className="font-medium text-gray-900 dark:text-slate-100">{log.user_full_name || '-'}</p>
            <p className="text-xs text-gray-400 dark:text-slate-500 break-all">{log.user_email}</p>
          </div>
          <div>
            <p className="text-gray-500 dark:text-slate-400">{t('audit.ipAddress')}</p>
            <p className="font-medium text-gray-900 dark:text-slate-100">{log.ip_address || '-'}</p>
          </div>
          <div>
            <p className="text-gray-500 dark:text-slate-400">{t('audit.entityType')}</p>
            <p className="font-medium text-gray-900 dark:text-slate-100">{entityLabel ?? formatEntityType(log.entity_type)}</p>
          </div>
          <div>
            <p className="text-gray-500 dark:text-slate-400">{t('audit.action')}</p>
            <ActionBadge action={log.action} label={actionLabel} />
          </div>
          <div className="sm:col-span-2">
            <p className="text-gray-500 dark:text-slate-400">{t('audit.entityId')}</p>
            <p className="font-mono text-xs text-gray-800 dark:text-slate-200 break-all">{log.entity_id || '-'}</p>
          </div>
          <div className="sm:col-span-2">
            <p className="text-gray-500 dark:text-slate-400">{t('common.description')}</p>
            <p className="font-medium text-gray-900 dark:text-slate-100 break-words">{log.description || '-'}</p>
          </div>
          <div className="sm:col-span-2">
            <p className="text-gray-500 dark:text-slate-400">{t('audit.userAgent')}</p>
            <p className="text-xs text-gray-600 dark:text-slate-300 break-all">{log.user_agent || '-'}</p>
          </div>
        </div>

        {diffRows.length > 0 && (
          <div className="space-y-3">
            <h3 className="text-sm font-medium text-gray-700 dark:text-slate-300">{t('audit.changesTitle')}</h3>
            <div className="border border-gray-200 dark:border-slate-700 rounded-xl overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="bg-gray-50 dark:bg-slate-800/70 border-b border-gray-200 dark:border-slate-700">
                    <th className="text-left px-3 py-2 font-medium text-gray-600 dark:text-slate-300">{t('audit.field')}</th>
                    <th className="text-left px-3 py-2 font-medium text-gray-600 dark:text-slate-300">{t('audit.previousValues')}</th>
                    <th className="text-left px-3 py-2 font-medium text-gray-600 dark:text-slate-300">{t('audit.newValues')}</th>
                  </tr>
                </thead>
                <tbody>
                  {diffRows.map((row) => (
                    <tr
                      key={row.key}
                      data-changed={row.changed ? 'true' : 'false'}
                      className={`border-b border-gray-100 dark:border-slate-800 last:border-0 ${
                        row.changed ? 'bg-amber-50/40 dark:bg-amber-500/5' : ''
                      }`}
                    >
                      <td className="px-3 py-2 font-mono font-medium text-gray-800 dark:text-slate-200 align-top whitespace-nowrap">
                        {row.key}
                      </td>
                      <td
                        className={`px-3 py-2 font-mono break-all align-top ${
                          row.changed && row.oldVal !== undefined
                            ? 'text-red-700 dark:text-red-300 bg-red-50/60 dark:bg-red-500/10'
                            : 'text-gray-500 dark:text-slate-400'
                        }`}
                      >
                        {formatAuditValue(row.oldVal)}
                      </td>
                      <td
                        className={`px-3 py-2 font-mono break-all align-top ${
                          row.changed && row.newVal !== undefined
                            ? 'text-green-700 dark:text-emerald-300 bg-green-50/60 dark:bg-emerald-500/10'
                            : 'text-gray-500 dark:text-slate-400'
                        }`}
                      >
                        {formatAuditValue(row.newVal)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <details className="text-xs">
              <summary className="cursor-pointer text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 select-none">
                {t('audit.rawJson')}
              </summary>
              <div className="mt-2 space-y-2">
                {hasOld && (
                  <pre className="bg-red-50 dark:bg-red-500/10 text-red-900 dark:text-red-200 rounded-lg p-3 overflow-x-auto max-h-40">
                    {JSON.stringify(log.old_values, null, 2)}
                  </pre>
                )}
                {hasNew && (
                  <pre className="bg-green-50 dark:bg-emerald-500/10 text-green-900 dark:text-emerald-200 rounded-lg p-3 overflow-x-auto max-h-40">
                    {JSON.stringify(log.new_values, null, 2)}
                  </pre>
                )}
              </div>
            </details>
          </div>
        )}
      </div>
    </Modal>
  );
}
