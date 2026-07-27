import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ErrorBoundary } from '@/components/ErrorBoundary';

const RELOAD_MARKER_KEY = 'chunk-reload-at';

function Boom({ error }: { error: Error }): never {
  throw error;
}

function staleChunkError() {
  return new Error('Failed to fetch dynamically imported module: /assets/Reports-abc123.js');
}

beforeEach(() => {
  sessionStorage.clear();
  // React itself logs every caught render error; without this the suite output
  // is unreadable and a genuine warning is impossible to spot.
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  sessionStorage.clear();
});

describe('ErrorBoundary', () => {
  it('renders a recoverable fallback instead of blanking the tree', () => {
    const reload = vi.fn();
    render(
      <ErrorBoundary reload={reload}>
        <Boom error={new Error('render exploded')} />
      </ErrorBoundary>,
    );

    expect(screen.getByRole('heading', { name: 'Something went wrong' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Reload/ })).toBeInTheDocument();
    expect(console.error).toHaveBeenCalled();
    // A plain render bug is not a stale deploy: reloading would just crash again.
    expect(reload).not.toHaveBeenCalled();
  });

  it('renders children untouched when nothing throws', () => {
    render(
      <ErrorBoundary>
        <p>All good</p>
      </ErrorBoundary>,
    );

    expect(screen.getByText('All good')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Something went wrong' })).not.toBeInTheDocument();
  });

  it('reloads once for a stale chunk and records the attempt', () => {
    const reload = vi.fn();
    render(
      <ErrorBoundary reload={reload} now={() => 1_000}>
        <Boom error={staleChunkError()} />
      </ErrorBoundary>,
    );

    expect(reload).toHaveBeenCalledTimes(1);
    expect(sessionStorage.getItem(RELOAD_MARKER_KEY)).toBe('1000');
  });

  it('falls back rather than reloading again while the marker is fresh', () => {
    sessionStorage.setItem(RELOAD_MARKER_KEY, '1000');
    const reload = vi.fn();

    render(
      <ErrorBoundary reload={reload} now={() => 3_000}>
        <Boom error={staleChunkError()} />
      </ErrorBoundary>,
    );

    // The loop guard is the assertion that matters: a genuinely broken deploy
    // must degrade to a page that says so, never to an endless reload.
    expect(reload).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', { name: 'Something went wrong' })).toBeInTheDocument();
  });

  it('reloads again once the guard window has passed', () => {
    sessionStorage.setItem(RELOAD_MARKER_KEY, '1000');
    const reload = vi.fn();

    render(
      <ErrorBoundary reload={reload} now={() => 60_000}>
        <Boom error={staleChunkError()} />
      </ErrorBoundary>,
    );

    expect(reload).toHaveBeenCalledTimes(1);
  });

  it('recognises the Chrome ChunkLoadError shape', () => {
    const reload = vi.fn();
    const error = new Error('Loading chunk 42 failed.');
    error.name = 'ChunkLoadError';

    render(
      <ErrorBoundary reload={reload} now={() => 1_000}>
        <Boom error={error} />
      </ErrorBoundary>,
    );

    expect(reload).toHaveBeenCalledTimes(1);
  });

  it('clears the marker on a healthy mount so the next deploy still recovers', () => {
    sessionStorage.setItem(RELOAD_MARKER_KEY, '1000');

    render(
      <ErrorBoundary>
        <p>All good</p>
      </ErrorBoundary>,
    );

    expect(sessionStorage.getItem(RELOAD_MARKER_KEY)).toBeNull();
  });

  it('clears the error when remounted under a new key', () => {
    const { rerender } = render(
      <ErrorBoundary key="/reports" reload={vi.fn()}>
        <Boom error={new Error('render exploded')} />
      </ErrorBoundary>,
    );

    expect(screen.getByRole('heading', { name: 'Something went wrong' })).toBeInTheDocument();

    // What navigating to another route does: a fresh boundary instance, so the
    // user is not stuck in the fallback for the rest of the session.
    rerender(
      <ErrorBoundary key="/employees" reload={vi.fn()}>
        <p>Employees</p>
      </ErrorBoundary>,
    );

    expect(screen.getByText('Employees')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Something went wrong' })).not.toBeInTheDocument();
  });

  it('reloads from the fallback button', async () => {
    const user = userEvent.setup();
    const reload = vi.fn();

    render(
      <ErrorBoundary reload={reload}>
        <Boom error={new Error('render exploded')} />
      </ErrorBoundary>,
    );

    await user.click(screen.getByRole('button', { name: /Reload/ }));

    expect(reload).toHaveBeenCalledTimes(1);
  });
});
