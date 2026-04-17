import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Fingerprint, Plus, Trash2, Pencil, Check, X } from 'lucide-react';
import { listPasskeys, deletePasskey, renamePasskey, passkeyRegisterBegin, passkeyRegisterComplete } from '@/api/passkey';
import type { PasskeyInfo } from '@/api/passkey';
import { createPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';
import { formatDate } from '@/lib/utils';

export function PasskeyManagement() {
  const queryClient = useQueryClient();
  const [registering, setRegistering] = useState(false);
  const [newName, setNewName] = useState('');
  const [showNameInput, setShowNameInput] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  const { data: passkeys, isLoading } = useQuery({
    queryKey: ['passkeys'],
    queryFn: listPasskeys,
  });

  const deleteMutation = useMutation({
    mutationFn: deletePasskey,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setSuccess('Passkey deleted');
      setTimeout(() => setSuccess(''), 3000);
    },
  });

  const renameMutation = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) => renamePasskey(id, name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setEditingId(null);
    },
  });

  if (!isWebAuthnSupported()) {
    return null;
  }

  const handleRegister = async () => {
    setError('');
    setRegistering(true);
    try {
      // Step 1: Get challenge from server
      const { challenge_id, options } = await passkeyRegisterBegin();

      // Step 2: Browser WebAuthn ceremony
      const credential = await createPasskeyCredential(options);

      // Step 3: Send credential to server
      const name = newName.trim() || 'My Passkey';
      await passkeyRegisterComplete(challenge_id, credential, name);

      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setShowNameInput(false);
      setNewName('');
      setSuccess('Passkey registered successfully!');
      setTimeout(() => setSuccess(''), 3000);
    } catch (err: unknown) {
      console.error('Passkey registration error:', err);
      let msg = 'Failed to register passkey';
      if (err instanceof Error) msg = err.message;
      // Handle axios error if necessary
      if (typeof err === 'object' && err !== null && 'response' in err) {
        const axiosErr = err as { response: { data: { error?: string } } };
        if (axiosErr.response?.data?.error) msg = axiosErr.response.data.error;
      }
      setError(msg);
    } finally {
      setRegistering(false);
    }
  };

  return (
    <div className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <Fingerprint className="w-4 h-4 text-gray-400" />
          <span className="section-title">Passkeys</span>
        </div>
      </div>

      <p className="text-sm text-gray-500 mb-4">
        Use passkeys for faster, passwordless sign-in with your fingerprint, face, or device PIN.
      </p>

      {error && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl mb-4">{error}</div>
      )}
      {success && (
        <div className="bg-green-50 text-green-600 text-sm px-4 py-3 rounded-xl mb-4">{success}</div>
      )}

      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
        </div>
      ) : (
        <div className="space-y-2 mb-4">
          {(!passkeys || passkeys.length === 0) ? (
            <p className="text-sm text-gray-400 py-4 text-center">No passkeys registered yet.</p>
          ) : (
            passkeys.map((pk: PasskeyInfo) => (
              <div
                key={pk.id}
                className="flex items-center gap-3 p-3 rounded-xl border border-gray-100 hover:bg-gray-50"
              >
                <Fingerprint className="w-5 h-5 text-gray-400 shrink-0" />
                <div className="flex-1 min-w-0">
                  {editingId === pk.id ? (
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        className="border px-2 py-1 rounded-lg text-sm flex-1 outline-none focus:border-black"
                        autoFocus
                      />
                      <button
                        onClick={() => renameMutation.mutate({ id: pk.id, name: editName })}
                        className="p-1 text-green-600 hover:bg-green-50 rounded"
                      >
                        <Check className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => setEditingId(null)}
                        className="p-1 text-gray-400 hover:bg-gray-100 rounded"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  ) : (
                    <>
                      <p className="text-sm font-medium text-gray-900 truncate">{pk.credential_name}</p>
                      <p className="text-xs text-gray-400">
                        Added {formatDate(pk.created_at)}
                        {pk.last_used_at && ` \u00b7 Last used ${formatDate(pk.last_used_at)}`}
                      </p>
                    </>
                  )}
                </div>
                {editingId !== pk.id && (
                  <div className="flex items-center gap-1 shrink-0">
                    <button
                      onClick={() => { setEditingId(pk.id); setEditName(pk.credential_name); }}
                      className="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg"
                      title="Rename"
                    >
                      <Pencil className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => {
                        if (confirm('Delete this passkey? You won\'t be able to use it to sign in anymore.')) {
                          deleteMutation.mutate(pk.id);
                        }
                      }}
                      className="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded-lg"
                      title="Delete"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
              </div>
            ))
          )}
        </div>
      )}

      {/* Register new passkey */}
      {showNameInput ? (
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Passkey name (e.g., MacBook, iPhone)"
            className="border px-3 py-2 rounded-lg text-sm flex-1 outline-none focus:border-black"
            autoFocus
          />
          <button
            onClick={handleRegister}
            disabled={registering}
            className="bg-black text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors"
          >
            {registering ? 'Registering...' : 'Register'}
          </button>
          <button
            onClick={() => { setShowNameInput(false); setNewName(''); }}
            className="p-2 text-gray-400 hover:text-gray-600"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      ) : (
        <button
          onClick={() => setShowNameInput(true)}
          className="flex items-center gap-2 text-sm font-medium text-black hover:text-gray-600 transition-colors"
        >
          <Plus className="w-4 h-4" />
          Add a passkey
        </button>
      )}
    </div>
  );
}
