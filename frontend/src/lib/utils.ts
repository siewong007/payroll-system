import { type ClassValue, clsx } from 'clsx';

// Simple cn utility without tailwind-merge for now
export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

/** Format sen (cents) to MYR display: "RM 1,234.56" */
export function formatMYR(sen: number): string {
  const ringgit = sen / 100;
  return `RM ${ringgit.toLocaleString('en-MY', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

/** Format date to DD/MM/YYYY */
export function formatDate(date: string | Date): string {
  const d = new Date(date);
  return d.toLocaleDateString('en-GB', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  });
}
/** Extract error message from Axios-style or standard Error objects */
export function getErrorMessage(err: unknown, fallback = 'Action failed'): string {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const axiosErr = err as { response: { data?: { error?: string } } };
    if (axiosErr.response?.data?.error) return axiosErr.response.data.error;
  }
  if (err instanceof Error) return err.message;
  return fallback;
}
