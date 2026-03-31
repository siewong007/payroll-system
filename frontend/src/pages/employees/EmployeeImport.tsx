import { useState, useCallback } from 'react';
import { useMutation } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { Upload, Download, CheckCircle, XCircle, AlertTriangle, ArrowLeft, FileSpreadsheet, Loader2 } from 'lucide-react';
import { downloadImportTemplate, validateImport, confirmImport } from '@/api/employees';
import type { ImportValidationResponse, ImportConfirmResponse } from '@/types';

type Step = 'upload' | 'preview' | 'confirm';

export function EmployeeImport() {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('upload');
  const [file, setFile] = useState<File | null>(null);
  const [validation, setValidation] = useState<ImportValidationResponse | null>(null);
  const [skipInvalid, setSkipInvalid] = useState(true);
  const [result, setResult] = useState<ImportConfirmResponse | null>(null);
  const [dragOver, setDragOver] = useState(false);

  const validateMutation = useMutation({
    mutationFn: validateImport,
    onSuccess: (data) => {
      setValidation(data);
      setStep('preview');
    },
  });

  const confirmMutation = useMutation({
    mutationFn: confirmImport,
    onSuccess: (data) => {
      setResult(data);
      setStep('confirm');
    },
  });

  const handleDownloadTemplate = async (format: 'csv' | 'xlsx') => {
    try {
      const blob = await downloadImportTemplate(format);
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `employee_import_template.${format}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // error handled by query client
    }
  };

  const handleFileSelect = useCallback((selectedFile: File) => {
    const validExts = ['.csv', '.xlsx', '.xls'];
    const ext = selectedFile.name.substring(selectedFile.name.lastIndexOf('.')).toLowerCase();
    if (!validExts.includes(ext)) {
      alert('Please select a .csv or .xlsx file');
      return;
    }
    setFile(selectedFile);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleFileSelect(e.dataTransfer.files[0]);
    }
  }, [handleFileSelect]);

  const handleUpload = () => {
    if (file) {
      validateMutation.mutate(file);
    }
  };

  const handleConfirm = () => {
    if (validation) {
      confirmMutation.mutate({
        session_id: validation.session_id,
        skip_invalid: skipInvalid,
      });
    }
  };

  return (
    <div>
      <div className="flex items-center gap-3 mb-6">
        <button
          onClick={() => navigate('/employees')}
          className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
        >
          <ArrowLeft className="w-5 h-5" />
        </button>
        <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Import Employees</h1>
      </div>

      {/* Steps indicator */}
      <div className="flex items-center gap-2 mb-8">
        {(['upload', 'preview', 'confirm'] as Step[]).map((s, i) => (
          <div key={s} className="flex items-center gap-2">
            <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium ${
              step === s ? 'bg-black text-white' :
              (i < ['upload', 'preview', 'confirm'].indexOf(step)) ? 'bg-green-100 text-green-700' :
              'bg-gray-100 text-gray-400'
            }`}>
              {i + 1}
            </div>
            <span className={`text-sm hidden sm:inline ${step === s ? 'font-medium text-gray-900' : 'text-gray-400'}`}>
              {s === 'upload' ? 'Upload' : s === 'preview' ? 'Preview' : 'Results'}
            </span>
            {i < 2 && <div className="w-8 h-px bg-gray-200" />}
          </div>
        ))}
      </div>

      {/* Step 1: Upload */}
      {step === 'upload' && (
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-lg font-semibold mb-4">Upload Employee File</h2>

          {/* Template download */}
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-6">
            <p className="text-sm text-blue-800 mb-3">
              Download a template file with the correct headers and a sample row.
            </p>
            <div className="flex gap-2">
              <button
                onClick={() => handleDownloadTemplate('xlsx')}
                className="flex items-center gap-2 bg-blue-600 text-white px-3 py-1.5 rounded-lg hover:bg-blue-700 transition-colors text-sm"
              >
                <Download className="w-4 h-4" />
                Excel Template
              </button>
              <button
                onClick={() => handleDownloadTemplate('csv')}
                className="flex items-center gap-2 border border-blue-300 text-blue-700 px-3 py-1.5 rounded-lg hover:bg-blue-100 transition-colors text-sm"
              >
                <Download className="w-4 h-4" />
                CSV Template
              </button>
            </div>
          </div>

          {/* Drop zone */}
          <div
            onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
            className={`border-2 border-dashed rounded-xl p-8 text-center transition-colors ${
              dragOver ? 'border-black bg-gray-50' : 'border-gray-300'
            }`}
          >
            {file ? (
              <div className="flex flex-col items-center gap-3">
                <FileSpreadsheet className="w-12 h-12 text-green-600" />
                <div>
                  <p className="font-medium text-gray-900">{file.name}</p>
                  <p className="text-sm text-gray-500">{(file.size / 1024).toFixed(1)} KB</p>
                </div>
                <button
                  onClick={() => setFile(null)}
                  className="text-sm text-red-600 hover:text-red-700"
                >
                  Remove
                </button>
              </div>
            ) : (
              <div className="flex flex-col items-center gap-3">
                <Upload className="w-12 h-12 text-gray-400" />
                <div>
                  <p className="text-gray-600">Drag and drop your file here, or</p>
                  <label className="text-black font-medium cursor-pointer hover:underline">
                    browse files
                    <input
                      type="file"
                      accept=".csv,.xlsx,.xls"
                      className="hidden"
                      onChange={(e) => e.target.files?.[0] && handleFileSelect(e.target.files[0])}
                    />
                  </label>
                </div>
                <p className="text-xs text-gray-400">Supports .csv and .xlsx files up to 20MB</p>
              </div>
            )}
          </div>

          {validateMutation.error && (
            <div className="mt-4 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">
              {(validateMutation.error as Error).message || 'Failed to validate file'}
            </div>
          )}

          <div className="mt-6 flex justify-end">
            <button
              onClick={handleUpload}
              disabled={!file || validateMutation.isPending}
              className="flex items-center gap-2 bg-black text-white px-6 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {validateMutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Validating...
                </>
              ) : (
                'Validate & Preview'
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Preview */}
      {step === 'preview' && validation && (
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-lg font-semibold mb-4">Validation Results</h2>

          {/* Summary */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-6">
            <div className="bg-gray-50 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-gray-900">{validation.total_rows}</p>
              <p className="text-xs text-gray-500">Total Rows</p>
            </div>
            <div className="bg-green-50 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-green-700">{validation.valid_rows}</p>
              <p className="text-xs text-green-600">Valid</p>
            </div>
            <div className="bg-red-50 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-red-700">{validation.error_rows}</p>
              <p className="text-xs text-red-600">Errors</p>
            </div>
            <div className="bg-yellow-50 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-yellow-700">{validation.duplicate_rows}</p>
              <p className="text-xs text-yellow-600">Duplicates</p>
            </div>
          </div>

          {/* Row details */}
          <div className="border border-gray-200 rounded-lg overflow-hidden mb-6">
            <div className="max-h-96 overflow-auto">
              <table className="w-full text-sm">
                <thead className="bg-gray-50 sticky top-0">
                  <tr>
                    <th className="px-3 py-2 text-left font-medium text-gray-600">Row</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600">Status</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600">Employee</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600">Details</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-100">
                  {validation.rows.map((row) => (
                    <tr key={row.row_number} className={
                      row.status === 'valid' ? 'bg-green-50/50' :
                      row.status === 'duplicate' ? 'bg-yellow-50/50' :
                      'bg-red-50/50'
                    }>
                      <td className="px-3 py-2 text-gray-600">{row.row_number}</td>
                      <td className="px-3 py-2">
                        {row.status === 'valid' && <CheckCircle className="w-4 h-4 text-green-600" />}
                        {row.status === 'error' && <XCircle className="w-4 h-4 text-red-600" />}
                        {row.status === 'duplicate' && <AlertTriangle className="w-4 h-4 text-yellow-600" />}
                      </td>
                      <td className="px-3 py-2">
                        <div className="font-medium">{row.data?.full_name || '-'}</div>
                        <div className="text-xs text-gray-400">{row.data?.employee_number || '-'}</div>
                      </td>
                      <td className="px-3 py-2">
                        {row.errors.length > 0 ? (
                          <ul className="text-xs space-y-0.5">
                            {row.errors.map((err, i) => (
                              <li key={i} className="text-red-600">
                                <span className="font-medium">{err.field}</span>: {err.message}
                              </li>
                            ))}
                          </ul>
                        ) : (
                          <span className="text-xs text-green-600">Ready to import</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Options */}
          {(validation.error_rows > 0 || validation.duplicate_rows > 0) && (
            <label className="flex items-center gap-2 mb-4 text-sm">
              <input
                type="checkbox"
                checked={skipInvalid}
                onChange={(e) => setSkipInvalid(e.target.checked)}
                className="rounded"
              />
              <span>Skip invalid rows and import only valid ones ({validation.valid_rows} rows)</span>
            </label>
          )}

          {confirmMutation.error && (
            <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">
              {(confirmMutation.error as Error).message || 'Failed to import'}
            </div>
          )}

          <div className="flex justify-between">
            <button
              onClick={() => { setStep('upload'); setValidation(null); setFile(null); }}
              className="text-sm text-gray-600 hover:text-gray-900"
            >
              Back to upload
            </button>
            <button
              onClick={handleConfirm}
              disabled={validation.valid_rows === 0 || confirmMutation.isPending}
              className="flex items-center gap-2 bg-black text-white px-6 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {confirmMutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Importing...
                </>
              ) : (
                <>Import {skipInvalid ? validation.valid_rows : validation.total_rows} Employees</>
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Results */}
      {step === 'confirm' && result && (
        <div className="bg-white rounded-xl border border-gray-200 p-6 text-center">
          <CheckCircle className="w-16 h-16 text-green-600 mx-auto mb-4" />
          <h2 className="text-xl font-semibold mb-2">Import Complete</h2>
          <p className="text-gray-600 mb-6">
            Successfully imported <span className="font-bold text-green-700">{result.imported_count}</span> employees.
            {result.skipped_count > 0 && (
              <> <span className="text-yellow-600">{result.skipped_count} rows were skipped.</span></>
            )}
          </p>

          {result.errors.length > 0 && (
            <div className="mb-6 text-left">
              <h3 className="text-sm font-medium text-red-700 mb-2">Failed rows:</h3>
              <div className="bg-red-50 rounded-lg p-3 text-sm text-red-700 max-h-40 overflow-auto">
                {result.errors.map((row) => (
                  <div key={row.row_number} className="mb-1">
                    Row {row.row_number}: {row.errors.map(e => `${e.field}: ${e.message}`).join(', ')}
                  </div>
                ))}
              </div>
            </div>
          )}

          <button
            onClick={() => navigate('/employees')}
            className="bg-black text-white px-6 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium"
          >
            View Employees
          </button>
        </div>
      )}
    </div>
  );
}
