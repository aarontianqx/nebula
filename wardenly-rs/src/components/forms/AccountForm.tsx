import { useState } from "react";
import { X } from "lucide-react";
import { useAccountStore, Account } from "../../stores/accountStore";

interface Props {
  account: Account | null;
  onClose: () => void;
}

function AccountForm({ account, onClose }: Props) {
  const { createAccount, updateAccount } = useAccountStore();
  const isEditing = account !== null;

  const [formData, setFormData] = useState({
    role_name: account?.role_name ?? "",
    user_name: account?.user_name ?? "",
    password: account?.password ?? "",
    server_id: account?.server_id ?? 1,
  });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (isEditing && account) {
      await updateAccount({
        ...account,
        ...formData,
      });
    } else {
      await createAccount(formData);
    }

    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-[60]">
      <div className="bg-[var(--color-bg-secondary)] rounded-lg w-[400px] shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
          <h3 className="font-semibold">
            {isEditing ? "Edit Account" : "Add Account"}
          </h3>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Role Name
            </label>
            <input
              type="text"
              value={formData.role_name}
              onChange={(e) =>
                setFormData({ ...formData, role_name: e.target.value })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              required
            />
          </div>

          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Username
            </label>
            <input
              type="text"
              value={formData.user_name}
              onChange={(e) =>
                setFormData({ ...formData, user_name: e.target.value })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              required
            />
          </div>

          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Password
            </label>
            <input
              type="password"
              value={formData.password}
              onChange={(e) =>
                setFormData({ ...formData, password: e.target.value })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              required
            />
          </div>

          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Server ID
            </label>
            <input
              type="number"
              value={formData.server_id}
              onChange={(e) =>
                setFormData({ ...formData, server_id: parseInt(e.target.value) || 1 })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              min={1}
              required
            />
          </div>

          {/* Buttons */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm bg-[var(--color-bg-tertiary)] rounded-md hover:bg-[var(--color-border)] transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="px-4 py-2 text-sm bg-[var(--color-accent)] text-white rounded-md hover:bg-[var(--color-accent-hover)] transition-colors"
            >
              {isEditing ? "Save" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default AccountForm;

