import { getErrorMessage } from '@/lib/utils';

export interface BulkFailure {
  id: string;
  message: string;
}

export interface BulkOutcome {
  succeeded: string[];
  failed: BulkFailure[];
}

/**
 * Run a per-id action over a selection and report what happened to each one.
 *
 * The point is that this **never rejects**: partial failure is a value, not an
 * exception. `Promise.all` short-circuits on the first rejection, so a bulk
 * cancel where one row was already processed skipped `onSuccess` entirely — no
 * invalidation fired, the selection was never reset, and the rows that *did*
 * succeed kept rendering their stale status next to the ones that failed. With
 * an outcome value the caller's refetch and reset are unconditional by
 * construction rather than by remembering to use `onSettled`.
 */
export async function runBulk(
  ids: string[],
  action: (id: string) => Promise<unknown>,
): Promise<BulkOutcome> {
  const results = await Promise.allSettled(ids.map((id) => action(id)));
  const succeeded: string[] = [];
  const failed: BulkFailure[] = [];

  results.forEach((result, index) => {
    if (result.status === 'fulfilled') {
      succeeded.push(ids[index]);
      return;
    }
    failed.push({ id: ids[index], message: getErrorMessage(result.reason, 'Request failed') });
  });

  return { succeeded, failed };
}

/**
 * One line an admin can act on, for the page's existing error banner.
 *
 * `verb` is the past participle of the action ("cancelled", "deleted"). Distinct
 * server messages are listed because "3 of 8 failed" alone tells nobody whether
 * to retry.
 */
export function summarizeBulkFailure(outcome: BulkOutcome, verb: string): string {
  if (outcome.failed.length === 0) {
    return '';
  }

  const total = outcome.succeeded.length + outcome.failed.length;
  const reasons = Array.from(new Set(outcome.failed.map((failure) => failure.message))).slice(0, 3);
  return `${outcome.failed.length} of ${total} could not be ${verb} — ${reasons.join('; ')}`;
}
