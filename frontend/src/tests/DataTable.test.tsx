import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { DataTable, type Column } from '@/components/ui/DataTable';

interface Row {
  id: string;
  name: string;
  department: string;
  net: number;
}

const columns: Column<Row>[] = [
  { key: 'name', header: 'Name', render: (row) => row.name, primary: true },
  { key: 'department', header: 'Department', render: (row) => row.department },
  {
    key: 'net',
    header: 'Net Pay',
    align: 'right',
    render: (row) => `RM ${row.net.toFixed(2)}`,
    summaryRender: (row) => `RM ${row.net.toFixed(2)} (net)`,
  },
];

function makeRows(count: number, prefix = 'emp'): Row[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `${prefix}-${i + 1}`,
    name: `Employee ${i + 1}`,
    department: i % 2 === 0 ? 'Engineering' : 'Finance',
    net: 1000 + i,
  }));
}

/** The desktop <table> and the mobile card list both render every row, so
 *  scoping queries to the table keeps assertions from double-counting. */
function table() {
  return screen.getByRole('table');
}

function bodyRows() {
  const body = table().querySelector('tbody');
  return within(body as HTMLElement).getAllByRole('row');
}

describe('DataTable rendering', () => {
  it('renders a header per column and a row per record', () => {
    render(<DataTable columns={columns} data={makeRows(3)} />);

    const headers = within(table()).getAllByRole('columnheader');
    expect(headers.map((h) => h.textContent)).toEqual(['Name', 'Department', 'Net Pay']);
    expect(bodyRows()).toHaveLength(3);
    expect(within(table()).getByText('RM 1000.00')).toBeInTheDocument();
  });

  it('applies per-column alignment classes', () => {
    render(<DataTable columns={columns} data={makeRows(1)} />);

    const cells = within(bodyRows()[0]).getAllByRole('cell');
    expect(cells[0].className).toContain('text-left');
    expect(cells[2].className).toContain('text-right');
  });

  it('shows the loading state instead of rows or the empty message', () => {
    render(<DataTable columns={columns} data={[]} isLoading emptyMessage="No employees" />);

    expect(screen.getAllByText('Loading...').length).toBeGreaterThan(0);
    expect(screen.queryByText('No employees')).not.toBeInTheDocument();
  });

  it('shows a custom empty message spanning every column', () => {
    render(<DataTable columns={columns} data={[]} emptyMessage="No employees" renderActions={() => null} />);

    const emptyCell = within(table()).getAllByRole('cell')[0];
    expect(emptyCell).toHaveTextContent('No employees');
    // 3 columns + the actions column.
    expect(emptyCell).toHaveAttribute('colspan', '4');
  });

  it('renders an actions column that does not open the summary modal', async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={makeRows(1)}
        renderActions={(row) => (
          <button type="button" onClick={() => onAction(row.id)}>
            Edit
          </button>
        )}
      />,
    );

    await user.click(within(table()).getByRole('button', { name: 'Edit' }));

    expect(onAction).toHaveBeenCalledWith('emp-1');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

describe('DataTable client-side pagination', () => {
  it('slices data to the current page and reports the visible range', async () => {
    const user = userEvent.setup();
    render(<DataTable columns={columns} data={makeRows(25)} perPage={10} />);

    expect(bodyRows()).toHaveLength(10);
    expect(screen.getByText(/Showing 1–10 of 25/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '3' }));

    // The last page is a partial page: 21–25, five rows.
    expect(bodyRows()).toHaveLength(5);
    expect(screen.getByText(/Showing 21–25 of 25/)).toBeInTheDocument();
  });

  it('disables the previous and next arrows at the range ends', async () => {
    const user = userEvent.setup();
    const { container } = render(<DataTable columns={columns} data={makeRows(25)} perPage={10} />);

    const arrows = () => {
      const nav = container.querySelectorAll('button');
      return { prev: nav[0], next: nav[nav.length - 1] };
    };

    expect(arrows().prev).toBeDisabled();
    expect(arrows().next).toBeEnabled();

    await user.click(screen.getByRole('button', { name: '3' }));

    expect(arrows().prev).toBeEnabled();
    expect(arrows().next).toBeDisabled();
  });

  it('hides pagination entirely when everything fits on one page', () => {
    render(<DataTable columns={columns} data={makeRows(4)} perPage={10} />);

    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  it('lists every page without ellipsis up to seven pages', () => {
    render(<DataTable columns={columns} data={makeRows(70)} perPage={10} />);

    expect(screen.getByRole('button', { name: '7' })).toBeInTheDocument();
    expect(screen.queryByText('...')).not.toBeInTheDocument();
  });

  it('collapses distant pages behind ellipses once the range grows', async () => {
    const user = userEvent.setup();
    render(<DataTable columns={columns} data={makeRows(200)} perPage={10} />);

    // On page 1 only the tail is collapsed; first and last are always reachable.
    expect(screen.getAllByText('...')).toHaveLength(1);
    expect(screen.getByRole('button', { name: '1' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '20' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '20' }));

    // On the last page the leading pages collapse instead.
    expect(screen.getAllByText('...')).toHaveLength(1);
    expect(screen.getByText(/Showing 191–200 of 200/)).toBeInTheDocument();
  });
});

describe('DataTable pagination after the data shrinks', () => {
  it('shows the surviving rows instead of an empty page with no pager', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<DataTable columns={columns} data={makeRows(30)} perPage={10} />);

    await user.click(screen.getByRole('button', { name: '3' }));
    expect(screen.getByText(/Showing 21–30 of 30/)).toBeInTheDocument();

    // A status filter that leaves eight rows. Slicing [20,30) out of an
    // eight-row array rendered "No data found" *and* unmounted the pager, so
    // there was no control left to get back to the matching rows.
    rerender(<DataTable columns={columns} data={makeRows(8)} perPage={10} />);

    expect(bodyRows()).toHaveLength(8);
    expect(screen.queryByText('No data found')).not.toBeInTheDocument();
    // One page now, so the pager is legitimately gone — and the page state was
    // reset, not merely clamped for the frame.
    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  it('clamps to the new last page rather than to the first', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<DataTable columns={columns} data={makeRows(30)} perPage={10} />);

    await user.click(screen.getByRole('button', { name: '3' }));

    rerender(<DataTable columns={columns} data={makeRows(15)} perPage={10} />);

    expect(screen.getByText(/Showing 11–15 of 15/)).toBeInTheDocument();
    expect(bodyRows()).toHaveLength(5);
    expect(within(table()).getByText('Employee 11')).toBeInTheDocument();
  });

  it('does not reset the page when only the array identity changes', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<DataTable columns={columns} data={makeRows(30)} perPage={10} />);

    await user.click(screen.getByRole('button', { name: '3' }));

    // Every consumer passes an inline `data={query.data ?? []}`, so a refetch
    // hands over a brand new array with the same contents. Keying the reset on
    // the derived page count is what stops that pinning the table to page 1.
    rerender(<DataTable columns={columns} data={makeRows(30)} perPage={10} />);

    expect(screen.getByText(/Showing 21–30 of 30/)).toBeInTheDocument();
  });
});

describe('DataTable server-side pagination', () => {
  it('asks the parent for the last page once the server reports a smaller total', () => {
    const onPageChange = vi.fn();
    const { rerender } = render(
      <DataTable
        columns={columns}
        data={makeRows(10)}
        total={95}
        page={10}
        onPageChange={onPageChange}
        perPage={10}
      />,
    );

    expect(onPageChange).not.toHaveBeenCalled();

    rerender(
      <DataTable
        columns={columns}
        data={makeRows(5)}
        total={25}
        page={10}
        onPageChange={onPageChange}
        perPage={10}
      />,
    );

    expect(onPageChange).toHaveBeenCalledTimes(1);
    expect(onPageChange).toHaveBeenCalledWith(3);
  });

  it('leaves the parent alone while a page is still loading', () => {
    const onPageChange = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={[]}
        total={25}
        page={10}
        onPageChange={onPageChange}
        perPage={10}
        isLoading
      />,
    );

    expect(onPageChange).not.toHaveBeenCalled();
  });

  it('leaves the parent alone until the server has reported a total', () => {
    const onPageChange = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={makeRows(4)}
        page={10}
        onPageChange={onPageChange}
        perPage={10}
      />,
    );

    // `totalItems` falls back to `data.length` here, which would otherwise clamp
    // the parent to page 1 on every in-flight refetch.
    expect(onPageChange).not.toHaveBeenCalled();
  });


  it('renders the supplied page verbatim and delegates page changes', async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    // Server returns only the current page, but `total` describes the full set.
    render(
      <DataTable
        columns={columns}
        data={makeRows(10, 'page2')}
        total={95}
        page={2}
        onPageChange={onPageChange}
        perPage={10}
      />,
    );

    // Data is NOT sliced again — that would blank out server-paged results.
    expect(bodyRows()).toHaveLength(10);
    expect(screen.getByText(/Showing 11–20 of 95/)).toBeInTheDocument();

    // With 10 pages the window collapses to 1–3 … 10, so the last page is the
    // far jump that is actually reachable from page 2.
    await user.click(screen.getByRole('button', { name: '10' }));
    expect(onPageChange).toHaveBeenCalledWith(10);
  });

  it('advances and rewinds through the arrows without exceeding the bounds', async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    const { container } = render(
      <DataTable
        columns={columns}
        data={makeRows(10)}
        total={30}
        page={2}
        onPageChange={onPageChange}
        perPage={10}
      />,
    );

    const buttons = container.querySelectorAll('button');
    await user.click(buttons[0]);
    expect(onPageChange).toHaveBeenLastCalledWith(1);

    await user.click(buttons[buttons.length - 1]);
    expect(onPageChange).toHaveBeenLastCalledWith(3);
  });
});

describe('DataTable summary modal', () => {
  it('opens on row click and prefers summaryRender over render', async () => {
    const user = userEvent.setup();
    render(<DataTable columns={columns} data={makeRows(2)} summaryTitle="Payroll item" />);

    await user.click(within(table()).getByText('Employee 2'));

    const dialog = screen.getByRole('dialog', { name: 'Payroll item' });
    expect(within(dialog).getByText('RM 1001.00 (net)')).toBeInTheDocument();
  });

  it('derives the modal title from the clicked row when given a function', async () => {
    const user = userEvent.setup();
    render(
      <DataTable columns={columns} data={makeRows(2)} summaryTitle={(row) => `Details — ${row.name}`} />,
    );

    await user.click(within(table()).getByText('Employee 2'));

    expect(screen.getByRole('dialog', { name: 'Details — Employee 2' })).toBeInTheDocument();
  });

  it('omits columns flagged hideInSummary', async () => {
    const user = userEvent.setup();
    const withHidden: Column<Row>[] = [
      ...columns,
      { key: 'internal', header: 'Internal Ref', render: () => 'REF-1', hideInSummary: true },
    ];
    render(<DataTable columns={withHidden} data={makeRows(1)} />);

    await user.click(within(table()).getByText('Employee 1'));

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).queryByText('Internal Ref')).not.toBeInTheDocument();
    expect(within(dialog).getByText('Department')).toBeInTheDocument();
  });

  it('does not open when row clicks are disabled', async () => {
    const user = userEvent.setup();
    render(<DataTable columns={columns} data={makeRows(1)} disableRowClick />);

    await user.click(within(table()).getByText('Employee 1'));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('lets custom summary content close the modal', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        columns={columns}
        data={makeRows(1)}
        renderSummary={(row, close) => (
          <div>
            <span>Custom view for {row.name}</span>
            <button type="button" onClick={close}>
              Dismiss
            </button>
          </div>
        )}
      />,
    );

    await user.click(within(table()).getByText('Employee 1'));
    expect(screen.getByText('Custom view for Employee 1')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    // The Modal unmounts through an AnimatePresence exit transition. Assert the
    // end state rather than the removal event: `user.click` flushes effects
    // inside act(), so the exit can already have finished by the time we look,
    // and waitForElementToBeRemoved treats an already-absent element as an error.
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('refreshes the open row when the underlying data is refetched', async () => {
    const user = userEvent.setup();

    function Harness() {
      const [data, setData] = useState<Row[]>(makeRows(2));
      return (
        <>
          <button type="button" onClick={() => setData((rows) => rows.map((r) => ({ ...r, net: r.net + 500 })))}>
            Refetch
          </button>
          <DataTable columns={columns} data={data} />
        </>
      );
    }

    render(<Harness />);
    await user.click(within(table()).getByText('Employee 1'));
    expect(within(screen.getByRole('dialog')).getByText('RM 1000.00 (net)')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Refetch' }));

    // A React Query refetch replaces row objects by identity. The modal must
    // follow the row by key, otherwise it keeps showing stale figures.
    expect(within(screen.getByRole('dialog')).getByText('RM 1500.00 (net)')).toBeInTheDocument();
  });

  it('closes itself when the open row disappears from the data', async () => {
    const user = userEvent.setup();

    function Harness() {
      const [data, setData] = useState<Row[]>(makeRows(2));
      return (
        <>
          <button type="button" onClick={() => setData((rows) => rows.slice(1))}>
            Delete first
          </button>
          <DataTable columns={columns} data={data} />
        </>
      );
    }

    render(<Harness />);
    await user.click(within(table()).getByText('Employee 1'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Delete first' }));

    // Closing here is indirect — the data change re-runs DataTable's effect,
    // which clears selectedRow — so the whole sequence lands inside the click's
    // act() flush more often than not, leaving no dialog to observe being
    // removed. Assert it is gone, which holds whether it went now or shortly.
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});

describe('DataTable selection', () => {
  function selectionCheckboxes() {
    return within(table()).getAllByRole('checkbox', { name: 'Select row' });
  }

  function selectAll() {
    return within(table()).getByRole('checkbox', { name: 'Select all visible rows' });
  }

  it('adds and removes a single row key', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(
      <DataTable columns={columns} data={makeRows(3)} selectable selectedRowKeys={[]} onSelectedRowKeysChange={onChange} />,
    );

    await user.click(selectionCheckboxes()[1]);
    expect(onChange).toHaveBeenLastCalledWith(['emp-2']);

    rerender(
      <DataTable
        columns={columns}
        data={makeRows(3)}
        selectable
        selectedRowKeys={['emp-2']}
        onSelectedRowKeysChange={onChange}
      />,
    );

    await user.click(selectionCheckboxes()[1]);
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it('selects and clears every selectable row on the page', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(
      <DataTable columns={columns} data={makeRows(3)} selectable selectedRowKeys={[]} onSelectedRowKeysChange={onChange} />,
    );

    await user.click(selectAll());
    expect(onChange).toHaveBeenLastCalledWith(['emp-1', 'emp-2', 'emp-3']);

    rerender(
      <DataTable
        columns={columns}
        data={makeRows(3)}
        selectable
        selectedRowKeys={['emp-1', 'emp-2', 'emp-3']}
        onSelectedRowKeysChange={onChange}
      />,
    );
    expect(selectAll()).toBeChecked();

    await user.click(selectAll());
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it('keeps selections made on other pages when toggling the current page', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    // Page 2 is displayed; 'emp-1' was selected earlier on page 1.
    render(
      <DataTable
        columns={columns}
        data={makeRows(2, 'page2')}
        total={40}
        page={2}
        onPageChange={vi.fn()}
        perPage={2}
        selectable
        selectedRowKeys={['emp-1']}
        onSelectedRowKeysChange={onChange}
      />,
    );

    await user.click(selectAll());

    // Select-all is scoped to visible rows and must not drop off-page keys.
    expect(onChange).toHaveBeenLastCalledWith(['emp-1', 'page2-1', 'page2-2']);
  });

  it('marks the header checkbox indeterminate on a partial selection', () => {
    render(
      <DataTable
        columns={columns}
        data={makeRows(3)}
        selectable
        selectedRowKeys={['emp-2']}
        onSelectedRowKeysChange={vi.fn()}
      />,
    );

    const header = selectAll() as HTMLInputElement;
    expect(header.indeterminate).toBe(true);
    expect(header.checked).toBe(false);
  });

  it('ignores rows excluded by isRowSelectable', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={makeRows(3)}
        selectable
        selectedRowKeys={[]}
        onSelectedRowKeysChange={onChange}
        isRowSelectable={(row) => row.id !== 'emp-2'}
      />,
    );

    expect(selectionCheckboxes()[1]).toBeDisabled();

    await user.click(selectAll());
    // A locked row (e.g. an already-paid payslip) must never be swept in.
    expect(onChange).toHaveBeenLastCalledWith(['emp-1', 'emp-3']);
  });

  it('disables select-all when no row on the page may be selected', () => {
    render(
      <DataTable
        columns={columns}
        data={makeRows(2)}
        selectable
        selectedRowKeys={[]}
        onSelectedRowKeysChange={vi.fn()}
        isRowSelectable={() => false}
      />,
    );

    expect(selectAll()).toBeDisabled();
  });

  it('deduplicates keys so a repeated selection cannot double-count', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={makeRows(2)}
        selectable
        selectedRowKeys={['emp-1']}
        onSelectedRowKeysChange={onChange}
      />,
    );

    await user.click(selectAll());

    expect(onChange).toHaveBeenLastCalledWith(['emp-1', 'emp-2']);
  });

  it('does not open the summary modal when a checkbox is clicked', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        columns={columns}
        data={makeRows(2)}
        selectable
        selectedRowKeys={[]}
        onSelectedRowKeysChange={vi.fn()}
      />,
    );

    await user.click(selectionCheckboxes()[0]);

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

describe('DataTable row keys', () => {
  it('falls back to the index when a row has no id', () => {
    const rows = [{ name: 'Unsaved draft' }, { name: 'Another draft' }];
    const draftColumns: Column<{ name: string }>[] = [
      { key: 'name', header: 'Name', render: (row) => row.name },
    ];

    expect(() => render(<DataTable columns={draftColumns} data={rows} />)).not.toThrow();
    expect(bodyRows()).toHaveLength(2);
  });

  it('uses a custom rowKey for selection identity', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <DataTable
        columns={columns}
        data={makeRows(2)}
        selectable
        selectedRowKeys={[]}
        onSelectedRowKeysChange={onChange}
        rowKey={(row) => `custom-${row.name}`}
      />,
    );

    await user.click(within(table()).getAllByRole('checkbox', { name: 'Select row' })[0]);

    expect(onChange).toHaveBeenLastCalledWith(['custom-Employee 1']);
  });
});
