/**
 * Shown-and-submitted overtime defaults. `TimeSelector` has no empty option —
 * it renders 09:00 for an empty value — so a form MUST hold whatever it
 * displays, or the submit guard silently fights the screen: the user sees
 * 18:00–19:00, state holds `''`, and Submit stays dead with no explanation.
 */
export const OT_DEFAULT_START = '18:00';
export const OT_DEFAULT_END = '19:00';

/**
 * Minutes between two `HH:MM` times, wrapping past midnight for a night shift,
 * rounded to the nearest half hour.
 *
 * `< 0`, not `<= 0`: equal times are a zero-length window. Treating them as a
 * wrap submitted a fabricated 24-hour day, which the `hours <= 24` CHECK
 * happily accepts — it is in range, it is just not what happened.
 */
export function calculateOvertimeHours(start: string, end: string): number {
  if (!start || !end) return 0;
  const [startHour, startMinute] = start.split(':').map(Number);
  const [endHour, endMinute] = end.split(':').map(Number);
  let diff = (endHour * 60 + endMinute) - (startHour * 60 + startMinute);
  if (diff < 0) diff += 24 * 60;
  return Math.round(diff / 30) * 0.5;
}

/** 1.0 — kept derived so the defaults and the readout can never disagree. */
export const OT_DEFAULT_HOURS = calculateOvertimeHours(OT_DEFAULT_START, OT_DEFAULT_END);
