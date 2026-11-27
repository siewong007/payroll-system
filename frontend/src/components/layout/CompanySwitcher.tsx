import { useQuery } from '@tanstack/react-query';
import { Building2, Check, ChevronDown } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { useAuth } from '@/context/AuthContext';
import { useTranslation } from 'react-i18next';
import { getMyCompanies } from '@/api/admin';
import { toast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';

export function CompanySwitcher() {
  const { user, switchCompany } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [switching, setSwitching] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const { data: companies } = useQuery({
    queryKey: ['my-companies'],
    queryFn: getMyCompanies,
    enabled: !!user,
  });

  // Close dropdown on outside click or Escape
  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onMouseDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  // Hide only when there are 0 companies, or 1 company that is already active.
  // An unassigned user with 1 accessible company still needs to select it.
  if (!companies || companies.length === 0 || (companies.length === 1 && user?.company_id === companies[0].id)) {
    return null;
  }

  const current = companies.find((c) => c.id === user?.company_id);

  const handleSwitch = async (companyId: string) => {
    if (companyId === user?.company_id) {
      setOpen(false);
      return;
    }
    setSwitching(true);
    setOpen(false);
    try {
      await switchCompany(companyId);
      navigate('/', { replace: true });
    } catch (err) {
      toast.error(getErrorMessage(err));
    } finally {
      setSwitching(false);
    }
  };

  return (
    <div ref={ref} className="relative px-2.5 pt-2.5 pb-1">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        disabled={switching}
        aria-expanded={open}
        aria-haspopup="listbox"
        className="w-full flex items-center gap-2 px-2.5 py-2 rounded-md border border-[var(--border)] bg-[var(--card)] hover:border-[var(--border-strong)] transition-all-fast text-left"
      >
        <Building2 className="w-3.5 h-3.5 text-[var(--accent-1)] shrink-0" />
        <span className="text-xs font-medium text-[var(--foreground)] truncate flex-1">
          {switching ? t('common.switching') : current?.name || t('common.selectCompany')}
        </span>
        <ChevronDown
          className={`w-3.5 h-3.5 text-[var(--muted-foreground)] transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <div className="animate-drop-in absolute left-2.5 right-2.5 top-full mt-1 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-md z-50 py-1 max-h-48 overflow-y-auto scrollbar-thin">
          {companies.map((c) => {
            const isCurrent = c.id === user?.company_id;
            return (
              <button
                key={c.id}
                onClick={() => handleSwitch(c.id)}
                className={`w-full flex items-center gap-2 text-left px-3 py-2 text-xs transition-colors ${
                  isCurrent
                    ? 'bg-[var(--accent-soft)] text-[var(--foreground)] font-medium'
                    : 'text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)]'
                }`}
              >
                <span className="truncate flex-1">{c.name}</span>
                {isCurrent && <Check className="w-3.5 h-3.5 text-[var(--accent-1)] shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
