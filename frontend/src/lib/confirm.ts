import { createContext, useContext } from 'react';

export type ConfirmVariant = 'danger' | 'warning' | 'primary';

export interface ConfirmOptions {
  title?: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: ConfirmVariant;
}

export type ConfirmFn = (options: ConfirmOptions | string) => Promise<boolean>;

export function normalizeConfirmOptions(input: ConfirmOptions | string): ConfirmOptions {
  return typeof input === 'string' ? { description: input, variant: 'danger' } : input;
}

const fallbackConfirm: ConfirmFn = async (input) => {
  const opts = normalizeConfirmOptions(input);
  if (typeof window !== 'undefined' && typeof window.confirm === 'function') {
    return window.confirm(opts.description);
  }
  return true;
};

export const ConfirmContext = createContext<ConfirmFn>(fallbackConfirm);

export function useConfirm(): ConfirmFn {
  return useContext(ConfirmContext);
}
