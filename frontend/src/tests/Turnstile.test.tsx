import { createRef } from 'react';
import { render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';

function mockTurnstile() {
  const api = {
    render: vi.fn().mockReturnValue('widget-1'),
    reset: vi.fn(),
    remove: vi.fn(),
    getResponse: vi.fn(),
  };
  window.turnstile = api;
  return api;
}

afterEach(() => {
  vi.unstubAllEnvs();
  delete window.turnstile;
  document.querySelectorAll('script[data-turnstile-widget]').forEach((s) => s.remove());
});

describe('TurnstileWidget', () => {
  it('renders nothing when the site key is unset', () => {
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', '');
    const { container } = render(<TurnstileWidget onVerify={() => {}} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders via window.turnstile and forwards the token to onVerify', async () => {
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', 'test-site-key');
    const api = mockTurnstile();
    const onVerify = vi.fn();

    render(<TurnstileWidget onVerify={onVerify} />);

    await waitFor(() => expect(api.render).toHaveBeenCalledOnce());
    const options = api.render.mock.calls[0][1] as Record<string, unknown>;
    expect(options.sitekey).toBe('test-site-key');

    (options.callback as (t: string) => void)('tok-abc');
    expect(onVerify).toHaveBeenCalledWith('tok-abc');
  });

  it('reset() delegates to turnstile.reset for the rendered widget', async () => {
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', 'test-site-key');
    const api = mockTurnstile();
    const ref = createRef<TurnstileWidgetRef>();

    render(<TurnstileWidget onVerify={() => {}} ref={ref} />);
    await waitFor(() => expect(api.render).toHaveBeenCalledOnce());

    ref.current?.reset();
    expect(api.reset).toHaveBeenCalledWith('widget-1');
  });

  it('removes the widget on unmount', async () => {
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', 'test-site-key');
    const api = mockTurnstile();

    const { unmount } = render(<TurnstileWidget onVerify={() => {}} />);
    await waitFor(() => expect(api.render).toHaveBeenCalledOnce());

    unmount();
    expect(api.remove).toHaveBeenCalledWith('widget-1');
  });
});
