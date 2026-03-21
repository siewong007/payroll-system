import { useState } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { Modal } from './Modal';

export interface Column<T> {
  key: string;
  header: string;
  align?: 'left' | 'center' | 'right';
  render: (row: T) => React.ReactNode;
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
}

export function DataTable<T>({
  columns,
  data,
  total,
  page: controlledPage,
  onPageChange,
  perPage = 10,
  isLoading,
  emptyMessage = 'No data found',
  emptyIcon,
  rowKey,
  summaryTitle = 'Record Details',
  renderSummary,
  renderSummaryFooter,
  disableRowClick = false,
  renderActions,
}: DataTableProps<T>) {
  const [selectedRow, setSelectedRow] = useState<T | null>(null);
  const [internalPage, setInternalPage] = useState(1);

  const isServerSide = controlledPage !== undefined && onPageChange !== undefined;
  const currentPage = isServerSide ? controlledPage : internalPage;
  const setCurrentPage = isServerSide ? onPageChange! : setInternalPage;

  const totalItems = isServerSide ? (total ?? data.length) : data.length;
  const totalPages = Math.ceil(totalItems / perPage);
  const displayData = isServerSide
    ? data
    : data.slice((currentPage - 1) * perPage, currentPage * perPage);

  const colCount = columns.length + (renderActions ? 1 : 0);

  const getKey = (row: T, index: number) => {
    if (rowKey) return rowKey(row, index);
    if ((row as Record<string, unknown>).id) return String((row as Record<string, unknown>).id);
    return String(index);
  };

  const handleRowClick = (row: T) => {
    if (!disableRowClick) setSelectedRow(row);
  };

  const modalTitle = typeof summaryTitle === 'function' && selectedRow
    ? summaryTitle(selectedRow)
    : (summaryTitle as string);

  // Split columns into primary (shown prominently on mobile cards) and secondary
  const primaryColumns = columns.filter((col) => col.primary);
  const secondaryColumns = columns.filter((col) => !col.primary);
  // If no columns are marked primary, use first 2 as primary
  const mobilePrimary = primaryColumns.length > 0 ? primaryColumns : columns.slice(0, 2);
  const mobileSecondary = primaryColumns.length > 0 ? secondaryColumns : columns.slice(2);

  const getPageNumbers = (): (number | 'ellipsis')[] => {
    if (totalPages <= 7) return Array.from({ length: totalPages }, (_, i) => i + 1);
    const pages: (number | 'ellipsis')[] = [1];
    if (currentPage > 3) pages.push('ellipsis');
    const start = Math.max(2, currentPage - 1);
    const end = Math.min(totalPages - 1, currentPage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (currentPage < totalPages - 2) pages.push('ellipsis');
    if (totalPages > 1) pages.push(totalPages);
    return pages;
  };

  return (
    <>
      <div className="bg-white rounded-2xl shadow overflow-hidden">
        {/* Mobile card view */}
        <div className="md:hidden">
          {isLoading ? (
            <div className="px-4 py-12 text-center text-gray-400">
              <div className="flex items-center justify-center gap-2">
                <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-gray-900" />
                <span>Loading...</span>
              </div>
            </div>
          ) : displayData.length === 0 ? (
            <div className="px-4 py-12 text-center text-gray-400">
              {emptyIcon && <div className="flex justify-center mb-2">{emptyIcon}</div>}
              {emptyMessage}
            </div>
          ) : (
            <div className="divide-y divide-gray-100">
              {displayData.map((row, idx) => (
                <div
                  key={getKey(row, idx)}
                  className={`p-4 ${!disableRowClick ? 'cursor-pointer active:bg-gray-50' : ''}`}
                  onClick={() => handleRowClick(row)}
                >
                  {/* Primary fields */}
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0 space-y-0.5">
                      {mobilePrimary.map((col) => (
                        <div key={col.key} className="text-sm text-gray-900">
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
                          <span className="text-[10px] font-medium text-gray-400 uppercase tracking-wider">{col.header}</span>
                          <div className="text-xs text-gray-600 truncate">{col.render(row)}</div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Desktop table view */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 text-left">
              <tr>
                {columns.map((col) => (
                  <th
                    key={col.key}
                    className={`px-6 py-3 text-xs font-medium text-gray-500 uppercase tracking-wide ${
                      col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'
                    }`}
                  >
                    {col.header}
                  </th>
                ))}
                {renderActions && (
                  <th className="px-6 py-3 text-xs font-medium text-gray-500 uppercase text-center">Actions</th>
                )}
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {isLoading ? (
                <tr>
                  <td colSpan={colCount} className="px-6 py-12 text-center text-gray-400">
                    <div className="flex items-center justify-center gap-2">
                      <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-gray-900" />
                      <span>Loading...</span>
                    </div>
                  </td>
                </tr>
              ) : displayData.length === 0 ? (
                <tr>
                  <td colSpan={colCount} className="px-6 py-12 text-center text-gray-400">
                    {emptyIcon && <div className="flex justify-center mb-2">{emptyIcon}</div>}
                    {emptyMessage}
                  </td>
                </tr>
              ) : (
                displayData.map((row, idx) => (
                  <tr
                    key={getKey(row, idx)}
                    className={`hover:bg-gray-50 transition-colors ${!disableRowClick ? 'cursor-pointer' : ''}`}
                    onClick={() => handleRowClick(row)}
                  >
                    {columns.map((col) => (
                      <td
                        key={col.key}
                        className={`px-6 py-4 text-sm ${
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

        {/* Pagination */}
        {totalPages > 1 && !isLoading && (
          <div className="flex flex-col gap-2 sm:flex-row items-center justify-between px-4 sm:px-6 py-3 border-t border-gray-100 bg-gray-50">
            <span className="text-sm text-gray-500">
              Showing {(currentPage - 1) * perPage + 1}–{Math.min(currentPage * perPage, totalItems)} of {totalItems}
            </span>
            <div className="flex items-center gap-1">
              <button
                onClick={() => setCurrentPage(Math.max(1, currentPage - 1))}
                disabled={currentPage === 1}
                className="p-2 sm:p-1.5 rounded-lg hover:bg-gray-200 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
              {/* Simplified pagination on mobile: just page count */}
              <span className="sm:hidden text-sm text-gray-600 px-2">
                {currentPage} / {totalPages}
              </span>
              {/* Full pagination on desktop */}
              <div className="hidden sm:flex items-center gap-1">
                {getPageNumbers().map((p, i) =>
                  p === 'ellipsis' ? (
                    <span key={`e${i}`} className="px-1 text-gray-400">...</span>
                  ) : (
                    <button
                      key={p}
                      onClick={() => setCurrentPage(p)}
                      className={`min-w-[32px] h-8 rounded-lg text-sm font-medium transition-colors ${
                        p === currentPage
                          ? 'bg-black text-white'
                          : 'text-gray-600 hover:bg-gray-200'
                      }`}
                    >
                      {p}
                    </button>
                  )
                )}
              </div>
              <button
                onClick={() => setCurrentPage(Math.min(totalPages, currentPage + 1))}
                disabled={currentPage === totalPages}
                className="p-2 sm:p-1.5 rounded-lg hover:bg-gray-200 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
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
        onClose={() => setSelectedRow(null)}
        title={modalTitle}
        footer={selectedRow && renderSummaryFooter ? renderSummaryFooter(selectedRow, () => setSelectedRow(null)) : undefined}
      >
        {selectedRow && (
          renderSummary ? (
            renderSummary(selectedRow, () => setSelectedRow(null))
          ) : (
            <div className="space-y-5">
              {columns
                .filter((col) => !col.hideInSummary)
                .map((col) => (
                  <div key={col.key} className="flex flex-col gap-1.5">
                    <span className="text-xs font-medium text-gray-400 uppercase tracking-wider">{col.header}</span>
                    <div className="text-sm text-gray-900">
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
