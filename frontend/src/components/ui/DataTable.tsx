import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowDown, ArrowUp, ArrowUpDown, ChevronLeft, ChevronRight } from 'lucide-react';
import { Modal } from './Modal';
import { Skeleton } from './Skeleton';
import { useMediaQuery } from '@/lib/useMediaQuery';

export interface Column<T> {
  key: string;
  header: string;
  align?: 'left' | 'center' | 'right';
  render: (row: T) => React.ReactNode;
  /** Optional value extractor enabling client-side sorting on this column. */
  sortValue?: (row: T) => string | number | null | undefined;
  /** Used in the summary modal. If omitted, render() is used. */
  summaryRender?: (row: T) => React.ReactNode;
  /** Hide this column from the summary modal */
  hideInSummary?: boolean;
  /** Show this column prominently in mobile card view (first 2-3 columns recommended) */
  primary?: boolean;
}

interface DataTableProps<T> {
  columns: Column<T>[];
  data: T[];
  /** Total count for server-side pagination. If omitted, client-side pagination is used. */
  total?: number;
  /** Current page (1-indexed) for server-side pagination */
  page?: number;
  /** Callback when page changes (server-side pagination) */
  onPageChange?: (page: number) => void;
  /** Rows per page. Defaults to 10. */
  perPage?: number;
  isLoading?: boolean;
  emptyMessage?: string;
  /** Custom icon shown when the table is empty */
  emptyIcon?: React.ReactNode;
  /** Optional action rendered beneath emptyMessage (e.g. Clear filters or Create CTA) */
  emptyAction?: React.ReactNode;
  /** Key extractor for each row. Defaults to (row as any).id */
  rowKey?: (row: T, index: number) => string;
  /** Title shown on the summary modal. Can be a function that receives the row. */
  summaryTitle?: string | ((row: T) => string);
  /** Custom summary content. If provided, overrides the default field list. Receives close callback. */
  renderSummary?: (row: T, close: () => void) => React.ReactNode;
  /** Footer rendered at the bottom of the modal (sticky, outside scroll area). Receives close callback. */
  renderSummaryFooter?: (row: T, close: () => void) => React.ReactNode;
  /** Disable row click modal */
  disableRowClick?: boolean;
  /** Extra action column rendered after all columns */
  renderActions?: (row: T) => React.ReactNode;
  /** Show row selection checkboxes and a select-all checkbox for the current page. */
  selectable?: boolean;
  /** Controlled list of selected row keys. */
  selectedRowKeys?: string[];
  /** Called when row selection changes. */
  onSelectedRowKeysChange?: (keys: string[]) => void;
  /** Disable selection for specific rows. Defaults to all rows selectable. */
  isRowSelectable?: (row: T) => boolean;
}

export function DataTable<T>({
  columns,
  data,
  total,
  page: controlledPage,
  onPageChange,
  perPage = 10,
  isLoading,
  emptyMessage,
  emptyIcon,
  emptyAction,
  rowKey,
  summaryTitle,
  renderSummary,
  renderSummaryFooter,
  disableRowClick = false,
  renderActions,
  selectable = false,
  selectedRowKeys = [],
  onSelectedRowKeysChange,
  isRowSelectable,
}: DataTableProps<T>) {
  const { t } = useTranslation();
  const [selectedSummary, setSelectedSummary] = useState<{ row: T; key: string } | null>(null);
  const selectedRow = selectedSummary?.row ?? null;
  const [internalPage, setInternalPage] = useState(1);
  const [sortState, setSortState] = useState<{ key: string; dir: 'asc' | 'desc' } | null>(null);
  const selectAllRef = useRef<HTMLInputElement>(null);
  const resolvedEmptyMessage = emptyMessage ?? t('common.noResults');
  const resolvedSummaryTitle = summaryTitle ?? t('common.recordDetails');

  const isServerSide = controlledPage !== undefined && onPageChange !== undefined;
  const currentPage = isServerSide ? controlledPage : internalPage;
  const setCurrentPage = isServerSide ? onPageChange! : setInternalPage;

  const sortedData = useMemo(() => {
    if (isServerSide || !sortState) return data;
    const col = columns.find((c) => c.key === sortState.key);
    if (!col?.sortValue) return data;
    const copy = [...data];
    copy.sort((a, b) => {
      const av = col.sortValue!(a);
      const bv = col.sortValue!(b);
      if (av == null && bv == null) return 0;
      if (av == null) return 1;
      if (bv == null) return -1;
      const cmp =
        typeof av === 'number' && typeof bv === 'number'
          ? av - bv
          : String(av).localeCompare(String(bv), undefined, { numeric: true, sensitivity: 'base' });
      return sortState.dir === 'asc' ? cmp : -cmp;
    });
    return copy;
  }, [columns, data, isServerSide, sortState]);

  const totalItems = isServerSide ? (total ?? data.length) : sortedData.length;
  const totalPages = Math.ceil(totalItems / perPage);
  // Clamp before slicing so no frame is ever blank.
  const safePage = Math.min(Math.max(1, currentPage), Math.max(1, totalPages));
  const displayData = useMemo(
    () => (isServerSide ? data : sortedData.slice((safePage - 1) * perPage, safePage * perPage)),
    [data, isServerSide, perPage, safePage, sortedData],
  );

  const colCount = columns.length + (renderActions ? 1 : 0) + (selectable ? 1 : 0);

  useEffect(() => {
    if (isServerSide) return;
    if (internalPage > totalPages) {
      setInternalPage(Math.max(1, totalPages));
    }
  }, [internalPage, isServerSide, totalPages]);

  useEffect(() => {
    if (!isServerSide || total === undefined || isLoading) return;
    const lastPage = Math.max(1, totalPages);
    if (currentPage > lastPage) {
      onPageChange!(lastPage);
    }
  }, [currentPage, isLoading, isServerSide, onPageChange, total, totalPages]);

  const getKey = useCallback((row: T, index: number) => {
    if (rowKey) return rowKey(row, index);
    if ((row as Record<string, unknown>).id) return String((row as Record<string, unknown>).id);
    return String(index);
  }, [rowKey]);

  const handleRowClick = (row: T, index: number) => {
    if (!disableRowClick) {
      setSelectedSummary({ row, key: getKey(row, index) });
    }
  };

  const handleRowKeyDown = (e: React.KeyboardEvent<HTMLElement>, row: T, index: number) => {
    if (disableRowClick || e.target !== e.currentTarget) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleRowClick(row, index);
    }
  };

  const toggleSort = (key: string) => {
    setSortState((prev) => {
      if (!prev || prev.key !== key) return { key, dir: 'asc' };
      if (prev.dir === 'asc') return { key, dir: 'desc' };
      return null;
    });
  };

  const selectedKeySet = new Set(selectedRowKeys);
  const selectableDisplayKeys = selectable
    ? displayData.flatMap((row, index) => ((isRowSelectable?.(row) ?? true) ? [getKey(row, index)] : []))
    : [];
  const selectedDisplayCount = selectableDisplayKeys.filter((key) => selectedKeySet.has(key)).length;
  const hasSelectableRows = selectableDisplayKeys.length > 0;
  const allDisplayRowsSelected = hasSelectableRows && selectedDisplayCount === selectableDisplayKeys.length;
  const someDisplayRowsSelected = selectedDisplayCount > 0 && !allDisplayRowsSelected;

  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = someDisplayRowsSelected;
    }
  }, [someDisplayRowsSelected]);

  const updateSelection = (keys: string[]) => {
    onSelectedRowKeysChange?.(Array.from(new Set(keys)));
  };

  const toggleRowSelection = (row: T, index: number) => {
    if (!(isRowSelectable?.(row) ?? true)) return;

    const key = getKey(row, index);
    updateSelection(
      selectedKeySet.has(key)
        ? selectedRowKeys.filter((selectedKey) => selectedKey !== key)
        : [...selectedRowKeys, key],
    );
  };

  const toggleDisplaySelection = () => {
    if (allDisplayRowsSelected) {
      updateSelection(selectedRowKeys.filter((key) => !selectableDisplayKeys.includes(key)));
      return;
    }

    updateSelection([...selectedRowKeys, ...selectableDisplayKeys]);
  };

  useEffect(() => {
    if (!selectedSummary) {
      return;
    }

    const nextSelectedRow =
      displayData.find((row, index) => getKey(row, index) === selectedSummary.key) ?? null;

    if (!nextSelectedRow) {
      setSelectedSummary(null);
      return;
    }

    if (nextSelectedRow !== selectedSummary.row) {
      setSelectedSummary({ row: nextSelectedRow, key: selectedSummary.key });
    }
  }, [displayData, getKey, selectedSummary]);

  const modalTitle = typeof resolvedSummaryTitle === 'function' && selectedRow
    ? resolvedSummaryTitle(selectedRow)
    : (resolvedSummaryTitle as string);

  const primaryColumns = columns.filter((col) => col.primary);
  const secondaryColumns = columns.filter((col) => !col.primary);
  const mobilePrimary = primaryColumns.length > 0 ? primaryColumns : columns.slice(0, 2);
  const mobileSecondary = primaryColumns.length > 0 ? secondaryColumns : columns.slice(2);

  const getPageNumbers = (): (number | 'ellipsis')[] => {
    if (totalPages <= 7) return Array.from({ length: totalPages }, (_, i) => i + 1);
    const pages: (number | 'ellipsis')[] = [1];
    if (safePage > 3) pages.push('ellipsis');
    const start = Math.max(2, safePage - 1);
    const end = Math.min(totalPages - 1, safePage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (safePage < totalPages - 2) pages.push('ellipsis');
    if (totalPages > 1) pages.push(totalPages);
    return pages;
  };

  const skeletonRows = Math.min(perPage, 5);
  const sortableColumns = !isServerSide ? columns.filter((c) => Boolean(c.sortValue)) : [];
  const mobileSortValue = sortState ? `${sortState.key}:${sortState.dir}` : '';
  const handleMobileSortChange = (raw: string) => {
    if (!raw) {
      setSortState(null);
      return;
    }
    const idx = raw.lastIndexOf(':');
    const key = raw.slice(0, idx);
    const dir = raw.slice(idx + 1) as 'asc' | 'desc';
    setSortState({ key, dir });
  };

  const isDesktop = useMediaQuery('(min-width: 768px)', true);

  return (
    <>
      <div className="card !p-0 overflow-hidden">
        {!isDesktop ? (
        /* Mobile card view */
        <div className="md:hidden">
          {!isLoading && (selectable || sortableColumns.length > 0) && displayData.length > 0 && (
            <div className="flex flex-wrap items-center justify-between gap-2 border-b border-gray-100 dark:border-slate-800 bg-gray-50/80 dark:bg-slate-900/60 px-4 py-2">
              {selectable ? (
                <label className="inline-flex min-h-11 items-center gap-2.5 text-xs font-medium text-gray-700 dark:text-slate-200 cursor-pointer">
                  <input
                    ref={selectAllRef}
                    type="checkbox"
                    checked={allDisplayRowsSelected}
                    disabled={!hasSelectableRows}
                    onChange={toggleDisplaySelection}
                    className="h-4 w-4 rounded border-gray-300 text-[var(--accent-1)] focus:ring-[var(--ring)] disabled:opacity-40"
                    aria-label={t('common.selectAll')}
                  />
                  <span>{t('common.selectAll')}</span>
                </label>
              ) : (
                <span />
              )}
              {sortableColumns.length > 0 && (
                <select
                  aria-label={t('ux.table.sortLabel')}
                  value={mobileSortValue}
                  onChange={(e) => handleMobileSortChange(e.target.value)}
                  className="min-h-10 rounded-lg border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-2.5 py-1 text-xs font-medium text-gray-700 dark:text-slate-200 focus:outline-none focus:ring-2 focus:ring-[var(--ring)]"
                >
                  <option value="">{t('ux.table.sortDefault')}</option>
                  {sortableColumns.map((col) => (
                    <optgroup key={col.key} label={col.header}>
                      <option value={`${col.key}:asc`}>
                        {t('ux.table.sortAscOption', { column: col.header })}
                      </option>
                      <option value={`${col.key}:desc`}>
                        {t('ux.table.sortDescOption', { column: col.header })}
                      </option>
                    </optgroup>
                  ))}
                </select>
              )}
            </div>
          )}
          {isLoading ? (
            <div className="divide-y divide-gray-100 dark:divide-slate-800 p-4 space-y-3" role="status">
              <span className="sr-only">{t('common.loading')}</span>
              {Array.from({ length: skeletonRows }, (_, idx) => (
                <div key={idx} className="pt-3 first:pt-0 space-y-2">
                  <Skeleton className="h-4 w-2/3" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              ))}
            </div>
          ) : displayData.length === 0 ? (
            <div className="px-4 py-12 text-center text-gray-400 dark:text-slate-400">
              {emptyIcon && <div className="flex justify-center mb-2">{emptyIcon}</div>}
              <p>{resolvedEmptyMessage}</p>
              {emptyAction && <div className="mt-3 flex justify-center">{emptyAction}</div>}
            </div>
          ) : (
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              {displayData.map((row, idx) => (
                <div
                  key={getKey(row, idx)}
                  tabIndex={!disableRowClick ? 0 : undefined}
                  onKeyDown={(e) => handleRowKeyDown(e, row, idx)}
                  className={`p-4 transition-colors ${
                    !disableRowClick
                      ? 'cursor-pointer active:bg-[var(--accent-soft)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--ring)]'
                      : ''
                  }`}
                  onClick={() => handleRowClick(row, idx)}
                >
                  {/* Primary fields */}
                  <div className="flex items-start justify-between gap-3">
                    {selectable && (
                      <div className="-m-2 inline-flex min-h-11 min-w-11 items-center justify-center shrink-0" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={selectedKeySet.has(getKey(row, idx))}
                          disabled={!(isRowSelectable?.(row) ?? true)}
                          onChange={() => toggleRowSelection(row, idx)}
                          className="h-4 w-4 rounded border-gray-300 text-[var(--accent-1)] focus:ring-[var(--ring)] disabled:opacity-40"
                          aria-label={t('common.selectRow')}
                        />
                      </div>
                    )}
                    <div className="flex-1 min-w-0 space-y-0.5">
                      {mobilePrimary.map((col) => (
                        <div key={col.key} className="text-sm text-gray-900 dark:text-slate-100">
                          {col.render(row)}
                        </div>
                      ))}
                    </div>
                    {renderActions && (
                      <div className="flex-shrink-0" onClick={(e) => e.stopPropagation()}>
                        {renderActions(row)}
                      </div>
                    )}
                  </div>
                  {/* Secondary fields */}
                  {mobileSecondary.length > 0 && (
                    <div className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1.5">
                      {mobileSecondary.map((col) => (
                        <div key={col.key} className="min-w-0">
                          <span className="text-[10px] font-medium text-gray-400 dark:text-slate-500 uppercase tracking-wider">{col.header}</span>
                          <div className="text-xs text-gray-600 dark:text-slate-300 truncate">{col.render(row)}</div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
        ) : (
        /* Desktop table view */
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50/80 dark:bg-slate-900/60 text-left border-b border-gray-100 dark:border-slate-800">
              <tr>
                {selectable && (
                  <th className="w-12 px-6 py-3">
                    <input
                      ref={selectAllRef}
                      type="checkbox"
                      checked={allDisplayRowsSelected}
                      disabled={!hasSelectableRows}
                      onChange={toggleDisplaySelection}
                      className="h-4 w-4 rounded border-gray-300 text-[var(--accent-1)] focus:ring-[var(--ring)] disabled:opacity-40"
                      aria-label={t('common.selectAllRows')}
                    />
                  </th>
                )}
                {columns.map((col) => {
                  const canSort = !isServerSide && Boolean(col.sortValue);
                  const activeSort = sortState?.key === col.key ? sortState.dir : null;
                  const ariaSort = canSort
                    ? activeSort === 'asc'
                      ? 'ascending'
                      : activeSort === 'desc'
                        ? 'descending'
                        : 'none'
                    : undefined;
                  return (
                    <th
                      key={col.key}
                      aria-sort={ariaSort}
                      className={`px-6 py-3 text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wide ${
                        col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'
                      }`}
                    >
                      {canSort ? (
                        <button
                          type="button"
                          onClick={() => toggleSort(col.key)}
                          aria-label={t('ux.table.sortBy', { column: col.header })}
                          className={`inline-flex items-center gap-1 rounded hover:text-gray-900 dark:hover:text-slate-100 transition-colors ${
                            col.align === 'right' ? 'ml-auto' : col.align === 'center' ? 'mx-auto' : ''
                          }`}
                        >
                          <span>{col.header}</span>
                          {activeSort === 'asc' ? (
                            <ArrowUp className="h-3.5 w-3.5 text-[var(--accent-1)]" aria-hidden="true" />
                          ) : activeSort === 'desc' ? (
                            <ArrowDown className="h-3.5 w-3.5 text-[var(--accent-1)]" aria-hidden="true" />
                          ) : (
                            <ArrowUpDown className="h-3.5 w-3.5 opacity-40" aria-hidden="true" />
                          )}
                        </button>
                      ) : (
                        col.header
                      )}
                    </th>
                  );
                })}
                {renderActions && (
                  <th className="px-6 py-3 text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase text-center">{t('common.actions')}</th>
                )}
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-slate-800/80">
              {isLoading ? (
                <tr>
                  <td colSpan={colCount} className="px-6 py-5">
                    <span className="sr-only">{t('common.loading')}</span>
                    <div className="space-y-3">
                      {Array.from({ length: skeletonRows }, (_, idx) => (
                        <div key={idx} className="flex items-center gap-4">
                          <Skeleton className="h-4 w-1/4" />
                          <Skeleton className="h-4 w-1/3" />
                          <Skeleton className="h-4 w-1/5 ml-auto" />
                        </div>
                      ))}
                    </div>
                  </td>
                </tr>
              ) : displayData.length === 0 ? (
                <tr>
                  <td colSpan={colCount} className="px-6 py-12 text-center text-gray-400 dark:text-slate-400">
                    {emptyIcon && <div className="flex justify-center mb-2">{emptyIcon}</div>}
                    {resolvedEmptyMessage}
                    {emptyAction && <div className="mt-3 flex justify-center">{emptyAction}</div>}
                  </td>
                </tr>
              ) : (
                displayData.map((row, idx) => (
                  <tr
                    key={getKey(row, idx)}
                    tabIndex={!disableRowClick ? 0 : undefined}
                    onKeyDown={(e) => handleRowKeyDown(e, row, idx)}
                    className={`hover:bg-[var(--accent-soft)] transition-colors ${
                      !disableRowClick
                        ? 'cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--ring)]'
                        : ''
                    }`}
                    onClick={() => handleRowClick(row, idx)}
                  >
                    {selectable && (
                      <td className="px-6 py-4" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={selectedKeySet.has(getKey(row, idx))}
                          disabled={!(isRowSelectable?.(row) ?? true)}
                          onChange={() => toggleRowSelection(row, idx)}
                          className="h-4 w-4 rounded border-gray-300 text-[var(--accent-1)] focus:ring-[var(--ring)] disabled:opacity-40"
                          aria-label={t('common.selectRow')}
                        />
                      </td>
                    )}
                    {columns.map((col) => (
                      <td
                        key={col.key}
                        className={`px-6 py-4 text-sm text-gray-800 dark:text-slate-200 ${
                          col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'
                        }`}
                      >
                        {col.render(row)}
                      </td>
                    ))}
                    {renderActions && (
                      <td className="px-6 py-4 text-sm text-center" onClick={(e) => e.stopPropagation()}>
                        {renderActions(row)}
                      </td>
                    )}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
        )}

        {/* Pagination */}
        {(totalPages > 1 || currentPage > 1) && !isLoading && (
          <div className="flex flex-col gap-2 sm:flex-row items-center justify-between px-4 sm:px-6 py-3 border-t border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-900/50">
            <span className="text-sm text-gray-500 dark:text-slate-400">
              {t('common.showing', { from: (safePage - 1) * perPage + 1, to: Math.min(safePage * perPage, totalItems), total: totalItems })}
            </span>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setCurrentPage(Math.max(1, safePage - 1))}
                disabled={safePage === 1}
                aria-label={t('a11y.previousPage')}
                className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center rounded-lg hover:bg-gray-200 dark:hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
              <span className="sm:hidden text-sm text-gray-600 dark:text-slate-300 px-2">
                {safePage} / {Math.max(1, totalPages)}
              </span>
              <div className="hidden sm:flex items-center gap-1">
                {getPageNumbers().map((p, i) =>
                  p === 'ellipsis' ? (
                    <span key={`e${i}`} className="px-1 text-gray-400">...</span>
                  ) : (
                    <button
                      type="button"
                      key={p}
                      onClick={() => setCurrentPage(p)}
                      aria-current={p === safePage ? 'page' : undefined}
                      className={`min-w-[32px] h-8 rounded-lg text-sm font-medium transition-all ${
                        p === safePage
                          ? 'bg-gradient-to-br from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-sm'
                          : 'text-gray-600 dark:text-slate-300 hover:bg-gray-200/80 dark:hover:bg-slate-800'
                      }`}
                    >
                      {p}
                    </button>
                  )
                )}
              </div>
              <button
                type="button"
                onClick={() => setCurrentPage(Math.min(Math.max(1, totalPages), safePage + 1))}
                disabled={safePage >= Math.max(1, totalPages)}
                aria-label={t('a11y.nextPage')}
                className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center rounded-lg hover:bg-gray-200 dark:hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Summary Modal */}
      <Modal
        open={selectedRow !== null}
        onClose={() => setSelectedSummary(null)}
        title={modalTitle}
        footer={selectedRow && renderSummaryFooter ? renderSummaryFooter(selectedRow, () => setSelectedSummary(null)) : undefined}
      >
        {selectedRow && (
          renderSummary ? (
            renderSummary(selectedRow, () => setSelectedSummary(null))
          ) : (
            <div className="space-y-5">
              {columns
                .filter((col) => !col.hideInSummary)
                .map((col) => (
                  <div key={col.key} className="flex flex-col gap-1.5">
                    <span className="text-xs font-medium text-gray-400 dark:text-slate-500 uppercase tracking-wider">{col.header}</span>
                    <div className="text-sm text-gray-900 dark:text-slate-100">
                      {col.summaryRender ? col.summaryRender(selectedRow) : col.render(selectedRow)}
                    </div>
                  </div>
                ))}
            </div>
          )
        )}
      </Modal>
    </>
  );
}
