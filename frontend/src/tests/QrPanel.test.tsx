import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const attendanceMocks = vi.hoisted(() => ({ generateQrToken: vi.fn() }));
const qrMocks = vi.hoisted(() => ({ toCanvas: vi.fn() }));

vi.mock('@/api/attendance', () => ({ generateQrToken: attendanceMocks.generateQrToken }));
vi.mock('qrcode', () => ({ default: { toCanvas: qrMocks.toCanvas } }));
vi.mock('@/pages/attendance/KioskCredentialsModal', () => ({
  KioskCredentialsModal: () => null,
}));

import { QrPanel } from '@/pages/attendance/QrPanel';

beforeEach(() => {
  attendanceMocks.generateQrToken.mockReset();
  qrMocks.toCanvas.mockReset();
});

describe('QrPanel QR canvas sizing', () => {
  it('clears fixed inline px dimensions written by qrcode after revealing QR code', async () => {
    attendanceMocks.generateQrToken.mockResolvedValue({
      token: 'tok-panel-1',
      scan_url: 'https://payroll.example/attendance/scan?token=tok-panel-1',
      expires_at: new Date(Date.now() + 300_000).toISOString(),
      ttl_seconds: 300,
    });
    qrMocks.toCanvas.mockImplementation((canvas: HTMLCanvasElement) => {
      canvas.width = 240;
      canvas.height = 240;
      canvas.style.width = '240px';
      canvas.style.height = '240px';
      return Promise.resolve();
    });

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <QrPanel canGenerate canManageKiosks={false} />
      </QueryClientProvider>,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Show QR code' }));

    const canvas = await waitFor(
      () => screen.getByRole('img', { name: 'Attendance QR Code' }) as HTMLCanvasElement,
    );
    await waitFor(() => {
      expect(qrMocks.toCanvas).toHaveBeenCalledTimes(1);
    });

    expect(canvas.style.width).toBe('');
    expect(canvas.style.height).toBe('');
    expect(canvas).toHaveClass('!w-full', '!h-auto', 'max-w-[240px]');
  });
});
