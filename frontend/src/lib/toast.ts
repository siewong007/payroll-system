import { createContext, useContext } from 'react';

export type ToastTone = 'success' | 'error' | 'warning' | 'info';

export interface ToastItem {
  id: string;
  tone: ToastTone;
  title?: string;
  message: string;
  durationMs?: number;
}

export interface PushToastInput {
  tone?: ToastTone;
  title?: string;
  message: string;
  durationMs?: number;
}

export interface ToastContextValue {
  push: (input: PushToastInput) => string;
  dismiss: (id: string) => void;
  success: (message: string, title?: string) => string;
  error: (message: string, title?: string) => string;
  warning: (message: string, title?: string) => string;
  info: (message: string, title?: string) => string;
}

type Listener = (item: ToastItem) => void;
const listeners = new Set<Listener>();
let nextToastSeq = 1;

export function allocateToastId(): string {
  return `toast-${nextToastSeq++}`;
}

export function subscribeImperativeToast(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function emitImperativeToast(input: PushToastInput): string {
  const id = allocateToastId();
  const item: ToastItem = {
    id,
    tone: input.tone ?? 'info',
    title: input.title,
    message: input.message,
    durationMs: input.durationMs ?? (input.tone === 'error' ? 6000 : 4200),
  };
  listeners.forEach((fn) => fn(item));
  return id;
}

/**
 * Imperative toast dispatcher — safe to call from non-React modules such as
 * QueryClient's `MutationCache.onError` handler in `App.tsx`.
 */
export const toast = {
  push: emitImperativeToast,
  success: (message: string, title?: string) => emitImperativeToast({ tone: 'success', message, title }),
  error: (message: string, title?: string) => emitImperativeToast({ tone: 'error', message, title }),
  warning: (message: string, title?: string) => emitImperativeToast({ tone: 'warning', message, title }),
  info: (message: string, title?: string) => emitImperativeToast({ tone: 'info', message, title }),
};

const fallbackContext: ToastContextValue = {
  push: emitImperativeToast,
  dismiss: () => undefined,
  success: toast.success,
  error: toast.error,
  warning: toast.warning,
  info: toast.info,
};

export const ToastContext = createContext<ToastContextValue>(fallbackContext);

export function useToast(): ToastContextValue {
  return useContext(ToastContext);
}
