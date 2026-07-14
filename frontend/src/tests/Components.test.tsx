import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Modal } from '@/components/ui/Modal';
import { TimeSelector } from '@/components/ui/TimeSelector';

describe('TimeSelector', () => {
  it('renders the controlled time and emits independently updated hour and minute values', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<TimeSelector label="Shift starts" value="09:30" minuteStep={15} onChange={onChange} />);
    const [hourSelect, minuteSelect] = screen.getAllByRole('combobox');

    expect(screen.getByText('09:30')).toBeInTheDocument();
    expect(minuteSelect.querySelectorAll('option')).toHaveLength(4);

    await user.selectOptions(hourSelect, '14');
    await user.selectOptions(minuteSelect, '45');

    expect(onChange).toHaveBeenNthCalledWith(1, '14:30');
    expect(onChange).toHaveBeenNthCalledWith(2, '09:45');
  });

  it('uses a safe default display for an empty value and disables both controls', () => {
    render(<TimeSelector label="Shift ends" value="" disabled onChange={vi.fn()} />);
    const controls = screen.getAllByRole('combobox');

    expect(screen.getByText('09:00')).toBeInTheDocument();
    expect(controls).toHaveLength(2);
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
