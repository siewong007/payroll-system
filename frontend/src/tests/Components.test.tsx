import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const uploadMocks = vi.hoisted(() => ({
  fetchUploadObjectUrl: vi.fn(),
}));

vi.mock('@/api/uploads', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/uploads')>()),
  fetchUploadObjectUrl: uploadMocks.fetchUploadObjectUrl,
}));

import { AttachmentLink } from '@/components/ui/AttachmentPreview';
import { Modal } from '@/components/ui/Modal';
import { TimeSelector } from '@/components/ui/TimeSelector';

beforeEach(() => {
  uploadMocks.fetchUploadObjectUrl.mockReset();
});

describe('TimeSelector', () => {
  it('renders the controlled time and emits independently updated hour and minute values', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<TimeSelector label="Shift starts" value="09:30" minuteStep={15} onChange={onChange} />);
    const [hourSelect, minuteSelect] = screen.getAllByRole('combobox');

    expect(hourSelect).toHaveValue('09');
    expect(minuteSelect).toHaveValue('30');
    expect(minuteSelect.querySelectorAll('option')).toHaveLength(4);

    await user.selectOptions(hourSelect, '14');
    await user.selectOptions(minuteSelect, '45');

    expect(onChange).toHaveBeenNthCalledWith(1, '14:30');
    expect(onChange).toHaveBeenNthCalledWith(2, '09:45');
  });

  it('uses a safe default display for an empty value and disables both controls', () => {
    render(<TimeSelector label="Shift ends" value="" disabled onChange={vi.fn()} />);
    const controls = screen.getAllByRole('combobox');

    expect(controls).toHaveLength(2);
    expect(controls[0]).toHaveValue('09');
    expect(controls[1]).toHaveValue('00');
    expect(controls[0]).toBeDisabled();
    expect(controls[1]).toBeDisabled();
  });
});

describe('Modal', () => {
  it('exposes dialog semantics, locks scrolling, and closes on Escape', () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <Modal open onClose={onClose} title="Employee details">
        <button type="button">Save</button>
      </Modal>,
    );

    const dialog = screen.getByRole('dialog', { name: 'Employee details' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(document.body.style.overflow).toBe('hidden');

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();

    rerender(
      <Modal open={false} onClose={onClose} title="Employee details">
        <button type="button">Save</button>
      </Modal>,
    );
    expect(document.body.style.overflow).toBe('');
  });

  it('closes from the overlay but not from a click inside the dialog', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="Confirm payroll">
        <button type="button">Confirm</button>
      </Modal>,
    );

    await user.click(screen.getByRole('button', { name: 'Confirm' }));
    expect(onClose).not.toHaveBeenCalled();

    const overlay = document.querySelector<HTMLElement>('[role="presentation"]');
    expect(overlay).not.toBeNull();
    await user.click(overlay as HTMLElement);
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe('AttachmentLink', () => {
  it('does not fetch managed uploads on mount and fetches only when clicked', async () => {
    const user = userEvent.setup();
    const fakeTab = { opener: {} as unknown, closed: false, location: { href: '' }, close: vi.fn() };
    const openSpy = vi.spyOn(window, 'open').mockReturnValue(fakeTab as unknown as Window);
    uploadMocks.fetchUploadObjectUrl.mockResolvedValue('blob:https://payrollmy.com/receipt-1');

    try {
      render(<AttachmentLink url="/api/uploads/receipt-1.pdf">View receipt</AttachmentLink>);

      expect(uploadMocks.fetchUploadObjectUrl).not.toHaveBeenCalled();

      await user.click(screen.getByRole('button', { name: 'View receipt' }));

      await waitFor(() =>
        expect(uploadMocks.fetchUploadObjectUrl).toHaveBeenCalledWith('/api/uploads/receipt-1.pdf'),
      );
      expect(fakeTab.opener).toBeNull();
      expect(fakeTab.location.href).toBe('blob:https://payrollmy.com/receipt-1');
    } finally {
      openSpy.mockRestore();
    }
  });
});
