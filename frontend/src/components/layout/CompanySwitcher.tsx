import { useQuery } from '@tanstack/react-query';
import { Building2, Check, ChevronDown } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { AnimatePresence, motion } from 'framer-motion';
import { useAuth } from '@/context/AuthContext';
import { getMyCompanies } from '@/api/admin';

export function CompanySwitcher() {
  const { user, switchCompany } = useAuth();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [switching, setSwitching] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const { data: companies } = useQuery({
    queryKey: ['my-companies'],
    queryFn: getMyCompanies,
    enabled: !!user,
  });

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  // Hide if user has only 0 or 1 companies — switcher would be a no-op.
  // Visible to super_admin and admin alike when they have multi-company access.
  if (!companies || companies.length <= 1) return null;

  const current = companies.find((c) => c.id === user?.company_id);

  const handleSwitch = async (companyId: string) => {
    if (companyId === user?.company_id) {
      setOpen(false);
      return;
    }
    setSwitching(true);
    try {
      await switchCompany(companyId);
      navigate('/', { replace: true });
    } finally {
      setSwitching(false);
      setOpen(false);
    }
  };

  return (
    <div ref={ref} className="relative px-3 mt-3 mb-1">
      <button
        onClick={() => setOpen(!open)}
        disabled={switching}
        className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-xl border border-white/10 bg-white/5 hover:bg-white/10 hover:border-white/20 transition-all-fast text-left"
      >
        <Building2 className="w-4 h-4 text-indigo-300 shrink-0" />
        <span className="text-sm font-medium text-slate-200 truncate flex-1">
          {switching ? 'Switching...' : current?.name || 'Select Company'}
        </span>
        <ChevronDown
          className={`w-4 h-4 text-slate-400 transition-transform duration-200 ${open ? 'rotate-180' : ''}`}
        />
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -6, scale: 0.98 }}
            transition={{ duration: 0.16, ease: 'easeOut' }}
            className="glass-dark absolute left-3 right-3 top-full mt-1.5 rounded-xl z-50 py-1 max-h-48 overflow-y-auto scrollbar-thin"
          >
            {companies.map((c) => {
              const isCurrent = c.id === user?.company_id;
              return (
                <button
                  key={c.id}
                  onClick={() => handleSwitch(c.id)}
                  className={`w-full flex items-center gap-2 text-left px-3 py-2.5 text-sm transition-colors ${
                    isCurrent
                      ? 'bg-indigo-500/15 text-indigo-200 font-medium'
                      : 'text-slate-300 hover:bg-white/5 hover:text-white'
                  }`}
                >
                  <span className="truncate flex-1">{c.name}</span>
                  {isCurrent && <Check className="w-3.5 h-3.5 text-indigo-300 shrink-0" />}
                </button>
              );
            })}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
