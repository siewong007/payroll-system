import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Search, Plus, FileText, AlertTriangle, Trash2, X, Lock, User } from 'lucide-react';
import { getDocuments, getDocumentCategories, createDocument, deleteDocument } from '@/api/documents';
import { getEmployees } from '@/api/employees';
import { formatDate } from '@/lib/utils';


function getStatusColor(status: string, expiryDate: string | null) {
  if (expiryDate && new Date(expiryDate) < new Date()) return 'bg-red-50 text-red-700';
  if (status === 'expired') return 'bg-red-50 text-red-700';
  if (status === 'archived') return 'bg-gray-100 text-gray-600';
  if (expiryDate) {
    const daysUntil = Math.ceil((new Date(expiryDate).getTime() - Date.now()) / 86400000);
    if (daysUntil <= 30) return 'bg-amber-50 text-amber-700';
  }
  return 'bg-green-50 text-green-700';
}

function getStatusLabel(status: string, expiryDate: string | null) {
  if (expiryDate && new Date(expiryDate) < new Date()) return 'Expired';
  if (status === 'expired') return 'Expired';
  if (status === 'archived') return 'Archived';
  if (expiryDate) {
    const daysUntil = Math.ceil((new Date(expiryDate).getTime() - Date.now()) / 86400000);
    if (daysUntil <= 30) return `Expiring (${daysUntil}d)`;
  }
  return 'Active';
}

export function DocumentList() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [selectedEmployee, setSelectedEmployee] = useState<{ id: string; name: string; number: string } | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const { data: employees, isLoading: loadingEmployees } = useQuery({
    queryKey: ['employees-select'],
    queryFn: () => getEmployees({ per_page: 200 }),
  });

  const filteredEmployees = search.trim()
    ? employees?.data.filter(emp =>
        emp.full_name.toLowerCase().includes(search.toLowerCase()) ||
        emp.employee_number.toLowerCase().includes(search.toLowerCase())
      )
    : employees?.data;

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Documents</h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
        >
          <Plus className="w-4 h-4" />
          Add Document
        </button>
      </div>

      {/* Search */}
      <div className="relative mb-6 max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
        <input
          type="text"
          placeholder="Search employee by name or ID..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full pl-10 pr-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none"
        />
      </div>

      {/* Employee List */}
      {loadingEmployees ? (
        <div className="flex items-center justify-center py-12">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
        </div>
      ) : !filteredEmployees || filteredEmployees.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 py-12 text-center text-gray-400">
          <User className="w-8 h-8 mx-auto mb-2 text-gray-300" />
          No employees found
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredEmployees.map((emp) => (
            <button
              key={emp.id}
              onClick={() => setSelectedEmployee({ id: emp.id, name: emp.full_name, number: emp.employee_number })}
              className="bg-white rounded-xl border border-gray-200 p-5 text-left hover:border-gray-300 hover:shadow-md transition-all group"
            >
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm group-hover:bg-gray-200 transition-colors">
                  {emp.full_name.split(' ').map(n => n[0]).slice(0, 2).join('')}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-sm text-gray-900 truncate">{emp.full_name}</div>
                  <div className="text-xs text-gray-400">{emp.employee_number}</div>
                </div>
                <FileText className="w-4 h-4 text-gray-300 group-hover:text-gray-500 transition-colors" />
              </div>
              {emp.department && (
                <div className="text-xs text-gray-400 mt-2">{emp.department}</div>
              )}
            </button>
          ))}
        </div>
      )}

      {/* Employee Documents Modal */}
      {selectedEmployee && (
        <EmployeeDocumentsModal
          employeeId={selectedEmployee.id}
          employeeName={selectedEmployee.name}
          employeeNumber={selectedEmployee.number}
          onClose={() => setSelectedEmployee(null)}
        />
      )}

      {/* Create Document Modal */}
      {showCreate && (
        <CreateDocumentModal
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false);
            queryClient.invalidateQueries({ queryKey: ['documents'] });
          }}
        />
      )}
    </div>
  );
}

function EmployeeDocumentsModal({
  employeeId,
  employeeName,
  employeeNumber,
  onClose,
}: {
  employeeId: string;
  employeeName: string;
  employeeNumber: string;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ['documents', { employee_id: employeeId }],
    queryFn: () => getDocuments({ employee_id: employeeId, per_page: 200 }),
  });

  const { data: categories } = useQuery({
    queryKey: ['document-categories'],
    queryFn: getDocumentCategories,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteDocument,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['documents'] });
    },
  });

  const categoryMap = new Map(categories?.map(c => [c.id, c.name]) ?? []);

  return (
    <>
      {/* Overlay */}
      <div className="fixed top-0 left-0 z-50 w-screen h-screen bg-black/50 backdrop-blur-sm" onClick={onClose} />

      {/* Centering container */}
      <div className="fixed top-0 left-0 z-50 w-screen h-screen grid place-items-center p-4 pointer-events-none">
        <div
          className="bg-white rounded-2xl shadow-2xl shadow-black/10 w-full max-w-4xl max-h-[85vh] flex flex-col pointer-events-auto"
          onClick={(e) => e.stopPropagation()}
        >
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-5 border-b border-gray-200 bg-gray-50/60 shrink-0 rounded-t-2xl">
          <div className="flex items-center gap-4">
            <div className="w-11 h-11 rounded-full bg-gray-100 flex items-center justify-center text-gray-900 font-semibold text-sm ring-2 ring-gray-200">
              {employeeName.split(' ').map(n => n[0]).slice(0, 2).join('')}
            </div>
            <div>
              <h2 className="text-lg font-bold text-gray-900">{employeeName}</h2>
              <p className="text-sm text-gray-400">{employeeNumber}</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setShowCreate(true)}
              className="flex items-center gap-1.5 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium shadow-sm"
            >
              <Plus className="w-3.5 h-3.5" />
              Add Document
            </button>
            <button onClick={onClose} className="p-2 hover:bg-gray-100 rounded-xl transition-colors">
              <X className="w-5 h-5 text-gray-400" />
            </button>
          </div>
        </div>

        {/* Documents */}
        <div className="overflow-y-auto flex-1">
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-black" />
            </div>
          ) : !data || data.data.length === 0 ? (
            <div className="text-center py-12 text-gray-400">
              <FileText className="w-8 h-8 mx-auto mb-2 text-gray-300" />
              <p className="text-sm">No documents for this employee</p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-gray-50 border-b border-gray-200 sticky top-0">
                <tr>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Title</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Category</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">File</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Issue Date</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Expiry</th>
                  <th className="text-center px-6 py-3 text-xs font-medium text-gray-500 uppercase">Status</th>
                  <th className="text-center px-6 py-3 text-xs font-medium text-gray-500 uppercase w-16"></th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {data.data.map((doc) => (
                  <tr key={doc.id} className="hover:bg-gray-50 transition-colors">
                    <td className="px-6 py-3">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm">{doc.title}</span>
                        {doc.is_confidential && <Lock className="w-3.5 h-3.5 text-amber-500" aria-label="Confidential" />}
                      </div>
                      {doc.description && (
                        <div className="text-xs text-gray-400 mt-0.5 truncate max-w-xs">{doc.description}</div>
                      )}
                    </td>
                    <td className="px-6 py-3 text-sm text-gray-600">
                      {doc.category_id ? categoryMap.get(doc.category_id) ?? '-' : '-'}
                    </td>
                    <td className="px-6 py-3">
                      <a
                        href={doc.file_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-sm text-gray-900 hover:text-black hover:underline"
                      >
                        {doc.file_name}
                      </a>
                      {doc.file_size && (
                        <div className="text-xs text-gray-400">{(doc.file_size / 1024).toFixed(0)} KB</div>
                      )}
                    </td>
                    <td className="px-6 py-3 text-sm text-gray-600">
                      {doc.issue_date ? formatDate(doc.issue_date) : '-'}
                    </td>
                    <td className="px-6 py-3 text-sm text-gray-600">
                      {doc.expiry_date ? (
                        <span className="flex items-center gap-1">
                          {new Date(doc.expiry_date) < new Date() && <AlertTriangle className="w-3.5 h-3.5 text-red-500" />}
                          {formatDate(doc.expiry_date)}
                        </span>
                      ) : '-'}
                    </td>
                    <td className="px-6 py-3 text-center">
                      <span className={`inline-flex px-2 py-1 rounded-full text-xs font-medium ${getStatusColor(doc.status, doc.expiry_date)}`}>
                        {getStatusLabel(doc.status, doc.expiry_date)}
                      </span>
                    </td>
                    <td className="px-6 py-3 text-center">
                      <button
                        onClick={() => {
                          if (confirm('Delete this document?')) deleteMutation.mutate(doc.id);
                        }}
                        className="p-1.5 text-gray-400 hover:text-red-600 rounded hover:bg-red-50 transition-colors"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {/* Footer */}
        {data && data.total > 0 && (
          <div className="px-8 py-3.5 border-t border-gray-200 text-sm text-gray-400 shrink-0 bg-gray-50/60 rounded-b-2xl">
            {data.total} document{data.total !== 1 ? 's' : ''}
          </div>
        )}

        {/* Nested Create Modal */}
        {showCreate && (
          <CreateDocumentModal
            defaultEmployeeId={employeeId}
            onClose={() => setShowCreate(false)}
            onCreated={() => {
              setShowCreate(false);
              queryClient.invalidateQueries({ queryKey: ['documents'] });
            }}
          />
        )}
        </div>
      </div>
    </>
  );
}

function CreateDocumentModal({
  defaultEmployeeId,
  onClose,
  onCreated,
}: {
  defaultEmployeeId?: string;
  onClose: () => void;
  onCreated: () => void;
}) {
  const { data: categories } = useQuery({
    queryKey: ['document-categories'],
    queryFn: getDocumentCategories,
  });

  const { data: employees } = useQuery({
    queryKey: ['employees-select'],
    queryFn: () => getEmployees({ per_page: 200 }),
  });

  const [form, setForm] = useState({
    title: '',
    description: '',
    file_name: '',
    file_url: '',
    category_id: '',
    issue_date: '',
    expiry_date: '',
    is_confidential: false,
  });
  const [employeeId, setEmployeeId] = useState(defaultEmployeeId ?? '');
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createDocument,
    onSuccess: onCreated,
    onError: () => setError('Failed to create document'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.title || !form.file_name || !form.file_url) {
      setError('Title, file name and file URL are required');
      return;
    }
    mutation.mutate({
      ...form,
      employee_id: employeeId || undefined,
      category_id: form.category_id || undefined,
      issue_date: form.issue_date || undefined,
      expiry_date: form.expiry_date || undefined,
    });
  };

  return (
    <>
      {/* Overlay */}
      <div className="fixed top-0 left-0 z-[60] w-screen h-screen bg-black/50 backdrop-blur-sm" />

      {/* Centering container */}
      <div className="fixed top-0 left-0 z-[60] w-screen h-screen grid place-items-center p-4 pointer-events-none">
        <div className="bg-white rounded-2xl shadow-2xl shadow-black/10 w-full max-w-xl max-h-[90vh] flex flex-col pointer-events-auto">
        <div className="flex items-center justify-between px-8 py-5 border-b border-gray-200 bg-gray-50/60 shrink-0 rounded-t-2xl">
          <h2 className="text-lg font-bold text-gray-900">Add Document</h2>
          <button onClick={onClose} className="p-2 hover:bg-gray-100 rounded-xl transition-colors">
            <X className="w-5 h-5 text-gray-400" />
          </button>
        </div>
        <form onSubmit={handleSubmit} className="overflow-y-auto flex-1 px-8 py-6 space-y-6">
          {error && <div className="text-sm text-red-600 bg-red-50 p-4 rounded-xl border border-red-100">{error}</div>}

          {/* Document Info */}
          <section className="bg-gray-50/50 rounded-xl border border-gray-100 p-5 space-y-5">
            <h3 className="text-xs font-bold text-gray-500 uppercase tracking-wider">Document Details</h3>
            <div>
              <label className="block text-sm font-medium text-gray-600 mb-1.5">Title *</label>
              <input
                type="text"
                value={form.title}
                onChange={(e) => setForm(f => ({ ...f, title: e.target.value }))}
                className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                placeholder="e.g. IC Copy"
              />
            </div>

            <div className="grid grid-cols-2 gap-5">
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">Employee</label>
                <select
                  value={employeeId}
                  onChange={(e) => setEmployeeId(e.target.value)}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                >
                  <option value="">Company-wide</option>
                  {employees?.data.map(emp => (
                    <option key={emp.id} value={emp.id}>{emp.full_name} ({emp.employee_number})</option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">Category</label>
                <select
                  value={form.category_id}
                  onChange={(e) => setForm(f => ({ ...f, category_id: e.target.value }))}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                >
                  <option value="">No category</option>
                  {categories?.map(c => (
                    <option key={c.id} value={c.id}>{c.name}</option>
                  ))}
                </select>
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-600 mb-1.5">Description</label>
              <textarea
                value={form.description}
                onChange={(e) => setForm(f => ({ ...f, description: e.target.value }))}
                className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                rows={2}
              />
            </div>
          </section>

          {/* File Info */}
          <section className="bg-gray-50/50 rounded-xl border border-gray-100 p-5 space-y-5">
            <h3 className="text-xs font-bold text-gray-500 uppercase tracking-wider">File Information</h3>
            <div className="grid grid-cols-2 gap-5">
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">File Name *</label>
                <input
                  type="text"
                  value={form.file_name}
                  onChange={(e) => setForm(f => ({ ...f, file_name: e.target.value }))}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                  placeholder="document.pdf"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">File URL *</label>
                <input
                  type="text"
                  value={form.file_url}
                  onChange={(e) => setForm(f => ({ ...f, file_url: e.target.value }))}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                  placeholder="/uploads/document.pdf"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-5">
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">Issue Date</label>
                <input
                  type="date"
                  value={form.issue_date}
                  onChange={(e) => setForm(f => ({ ...f, issue_date: e.target.value }))}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-600 mb-1.5">Expiry Date</label>
                <input
                  type="date"
                  value={form.expiry_date}
                  onChange={(e) => setForm(f => ({ ...f, expiry_date: e.target.value }))}
                  className="w-full px-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-black focus:border-black outline-none text-sm bg-white transition-colors"
                />
              </div>
            </div>

            <label className="flex items-center gap-2.5 cursor-pointer pt-1">
              <input
                type="checkbox"
                checked={form.is_confidential}
                onChange={(e) => setForm(f => ({ ...f, is_confidential: e.target.checked }))}
                className="rounded border-gray-300 w-4 h-4 text-gray-900 focus:ring-black"
              />
              <span className="text-sm font-medium text-gray-700">Confidential document</span>
            </label>
          </section>

          <div className="flex justify-end gap-3 pt-1">
            <button type="button" onClick={onClose} className="px-5 py-2.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg font-medium border border-gray-300 transition-colors">
              Cancel
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              className="px-6 py-2.5 text-sm bg-black text-white rounded-lg hover:bg-gray-800 disabled:opacity-50 font-medium shadow-sm transition-colors"
            >
              {mutation.isPending ? 'Creating...' : 'Create Document'}
            </button>
          </div>
        </form>
        </div>
      </div>
    </>
  );
}
