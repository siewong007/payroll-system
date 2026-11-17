import { createContext, useContext } from 'react';
import type { ConfirmVariant } from './confirm';

export interface PromptOptions {
  title?: string;
  description: string;
  placeholder?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  initialValue?: string;
  required?: boolean;
  variant?: ConfirmVariant;
}

export type PromptFn = (options: PromptOptions | string) => Promise<string | null>;

export function normalizePromptOptions(input: PromptOptions | string): PromptOptions {
  return typeof input === 'string' ? { description: input } : input;
}

const fallbackPrompt: PromptFn = async (input) => {
  const opts = normalizePromptOptions(input);
  if (typeof window !== 'undefined' && typeof window.prompt === 'function') {
    const raw = window.prompt(opts.description, opts.initialValue ?? '');
    if (raw === null) return null;
    const trimmed = raw.trim();
    if (opts.required && !trimmed) return null;
    return trimmed;
  }
  return null;
};

export const PromptContext = createContext<PromptFn>(fallbackPrompt);

export function usePrompt(): PromptFn {
  return useContext(PromptContext);
}
