import { describe, expect, it, vi } from 'vitest';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';

describe('runBulk', () => {
  it('splits a mixed batch and never rejects', async () => {
    const action = vi.fn(async (id: string) => {
      if (id === 'b') throw new Error('boom');
      return id;
    });

    const outcome = await runBulk(['a', 'b', 'c'], action);

    expect(outcome.succeeded).toEqual(['a', 'c']);
    expect(outcome.failed).toEqual([{ id: 'b', message: 'boom' }]);
    // The whole point: every id was attempted, not just the ones before the
    // first rejection.
    expect(action).toHaveBeenCalledTimes(3);
  });

  it('reads the server message out of an axios-shaped rejection', async () => {
    const outcome = await runBulk(['claim-1'], async () => {
      throw { response: { data: { error: 'This claim is already processed' } } };
    });

    expect(outcome.failed).toEqual([
      { id: 'claim-1', message: 'This claim is already processed' },
    ]);
  });

  it('returns two empty lists for an empty selection', async () => {
    const action = vi.fn();

    await expect(runBulk([], action)).resolves.toEqual({ succeeded: [], failed: [] });
    expect(action).not.toHaveBeenCalled();
  });

  it('pairs each failure back to its own id, not to a position in the results', async () => {
    const outcome = await runBulk(['a', 'b', 'c'], async (id) => {
      if (id === 'a' || id === 'c') throw new Error(`failed ${id}`);
      return id;
    });

    expect(outcome.succeeded).toEqual(['b']);
    expect(outcome.failed.map((failure) => failure.id)).toEqual(['a', 'c']);
    expect(outcome.failed[1].message).toBe('failed c');
  });
});

describe('summarizeBulkFailure', () => {
  it('is empty when nothing failed', () => {
    expect(summarizeBulkFailure({ succeeded: ['a'], failed: [] }, 'cancelled')).toBe('');
  });

  it('names the count and the distinct reasons', () => {
    const message = summarizeBulkFailure(
      {
        succeeded: ['a', 'b', 'c', 'd', 'e'],
        failed: [
          { id: 'f', message: 'This claim is already processed' },
          { id: 'g', message: 'This claim is already processed' },
          { id: 'h', message: 'Not found' },
        ],
      },
      'cancelled',
    );

    expect(message).toBe(
      '3 of 8 could not be cancelled — This claim is already processed; Not found',
    );
  });
});
