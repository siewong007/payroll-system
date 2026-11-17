import { useCallback, useMemo, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, HelpCircle, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Modal } from './Modal';
import {
  ConfirmContext,
  normalizeConfirmOptions,
  type ConfirmFn,
  type ConfirmOptions,
} from '@/lib/confirm';

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [active, setActive] = useState<ConfirmOptions | null>(null);
  const resolverRef = useRef<((confirmed: boolean) => void) | null>(null);

  const settle = useCallback((confirmed: boolean) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setActive(null);
    resolve?.(confirmed);
  }, []);

  const confirm = useCallback<ConfirmFn>((input) => {
    const opts = normalizeConfirmOptions(input);
    if (resolverRef.current) {
      resolverRef.current(false);
    }
    setActive(opts);
    return new Promise<boolean>((resolve) => {
      resolverRef.current = resolve;
    });
  }, []);

  const value = useMemo(() => confirm, [confirm]);

  const variant = active?.variant ?? 'danger';
  const resolvedTitle =
    active?.title ??
    (variant === 'danger' ? t('ux.confirm.destructiveTitle') : t('ux.confirm.defaultTitle'));
  const resolvedConfirm =
    active?.confirmLabel ??
    (variant === 'danger' ? t('common.delete') : t('ux.confirm.proceed'));
  const resolvedCancel = active?.cancelLabel ?? t('common.cancel');

  const Icon = variant === 'danger' ? Trash2 : variant === 'warning' ? AlertTriangle : HelpCircle;
  const badgeStyle =
    variant === 'danger'
      ? 'bg-red-50 text-red-600 ring-red-100 dark:bg-red-500/15 dark:text-red-300 dark:ring-red-500/20'
      : variant === 'warning'
        ? 'bg-amber-50 text-amber-600 ring-amber-100 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-500/20'
        : 'bg-indigo-50 text-indigo-600 ring-indigo-100 dark:bg-indigo-500/15 dark:text-indigo-300 dark:ring-indigo-500/20';

  const confirmButtonClass =
    variant === 'danger'
      ? 'inline-flex items-center justify-center gap-2 rounded-xl bg-red-600 px-4 py-2.5 min-h-[44px] text-sm font-semibold text-white shadow-sm transition-all hover:bg-red-700 active:scale-[0.98]'
      : variant === 'warning'
        ? 'inline-flex items-center justify-center gap-2 rounded-xl bg-amber-600 px-4 py-2.5 min-h-[44px] text-sm font-semibold text-white shadow-sm transition-all hover:bg-amber-700 active:scale-[0.98]'
        : 'btn-primary';

  return (
    <ConfirmContext.Provider value={value}>
      {children}
      <Modal
        open={active !== null}
        onClose={() => settle(false)}
        title={resolvedTitle}
        maxWidth="max-w-md"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => settle(false)} className="btn-secondary">
              {resolvedCancel}
            </button>
            <button type="button" onClick={() => settle(true)} className={confirmButtonClass}>
              {resolvedConfirm}
            </button>
          </div>
        }
      >
        <div className="flex items-start gap-3.5">
          <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ring-4 ${badgeStyle}`}>
            <Icon className="h-5 w-5" aria-hidden="true" />
          </div>
          <p className="pt-1 text-sm leading-relaxed text-gray-600 dark:text-slate-300">
            {active?.description}
          </p>
        </div>
      </Modal>
    </ConfirmContext.Provider>
  );
}
