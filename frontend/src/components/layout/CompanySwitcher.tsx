import { useQuery } from '@tanstack/react-query';
import { Building2, ChevronDown } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import { useAuth } from '@/context/AuthContext';
import { getMyCompanies } from '@/api/admin';

export function CompanySwitcher() {
  const { user, switchCompany } = useAuth();
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

  // Don't render for super_admin or if user has 0-1 companies
  if (user?.role === 'super_admin') return null;
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
    } finally {
      setSwitching(false);
      setOpen(false);
    }
  };

  return (
    <div ref={ref} className="relative px-3 mb-2">
      <button
        onClick={() => setOpen(!open)}
        disabled={switching}
        className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-xl border border-gray-200 bg-white hover:bg-gray-50 transition-colors text-left"
      >
        <Building2 className="w-4 h-4 text-gray-400 shrink-0" />
        <span className="text-sm font-medium text-gray-700 truncate flex-1">
          {switching ? 'Switching...' : current?.name || 'Select Company'}
        </span>
        <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>

      {open && (
        <div className="absolute left-3 right-3 top-full mt-1 bg-white rounded-xl border border-gray-200 shadow-lg z-50 py-1 max-h-48 overflow-y-auto">
          {companies.map((c) => (
            <button
              key={c.id}
              onClick={() => handleSwitch(c.id)}
              className={`w-full text-left px-3 py-2.5 text-sm transition-colors ${
                c.id === user?.company_id
                  ? 'bg-gray-50 text-gray-900 font-medium'
                  : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900'
              }`}
            >
              {c.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
