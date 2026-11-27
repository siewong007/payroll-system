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
  it('does not fetch managed uploads on mount and downloads non-image blobs on click without opening a tab', async () => {
    const user = userEvent.setup();
    const realCreateElement = document.createElement.bind(document);
    const anchor = realCreateElement('a');
    const anchorClick = vi.fn();
    anchor.click = anchorClick;
    const createElementSpy = vi
      .spyOn(document, 'createElement')
      .mockImplementation((tag: string) =>
        tag === 'a' ? anchor : realCreateElement(tag as 'div'),
      );
    const openSpy = vi.spyOn(window, 'open');
    uploadMocks.fetchUploadObjectUrl.mockResolvedValue('blob:https://payrollmy.com/receipt-1');

    try {
      render(<AttachmentLink url="/api/uploads/receipt-1.pdf">View receipt</AttachmentLink>);

      expect(uploadMocks.fetchUploadObjectUrl).not.toHaveBeenCalled();

      await user.click(screen.getByRole('button', { name: 'View receipt' }));

      await waitFor(() =>
        expect(uploadMocks.fetchUploadObjectUrl).toHaveBeenCalledWith('/api/uploads/receipt-1.pdf'),
      );
      expect(openSpy).not.toHaveBeenCalled();
      expect(anchor.getAttribute('href')).toBe('blob:https://payrollmy.com/receipt-1');
      expect(anchor.getAttribute('download')).toBe('receipt-1.pdf');
      expect(anchorClick).toHaveBeenCalledOnce();
    } finally {
      createElementSpy.mockRestore();
      openSpy.mockRestore();
    }
  });

  it('opens safe raster image uploads in a new tab with opener severed', async () => {
    const user = userEvent.setup();
    const fakeTab = { opener: {} as unknown, closed: false, location: { href: '' }, close: vi.fn() };
    const openSpy = vi.spyOn(window, 'open').mockReturnValue(fakeTab as unknown as Window);
    uploadMocks.fetchUploadObjectUrl.mockResolvedValue('blob:https://payrollmy.com/photo-1');

    try {
      render(<AttachmentLink url="/api/uploads/photo-1.png">View photo</AttachmentLink>);

      await user.click(screen.getByRole('button', { name: 'View photo' }));

      await waitFor(() =>
        expect(uploadMocks.fetchUploadObjectUrl).toHaveBeenCalledWith('/api/uploads/photo-1.png'),
      );
      expect(openSpy).toHaveBeenCalledWith('', '_blank');
      expect(fakeTab.opener).toBeNull();
      expect(fakeTab.location.href).toBe('blob:https://payrollmy.com/photo-1');
    } finally {
      openSpy.mockRestore();
    }
  });
});
