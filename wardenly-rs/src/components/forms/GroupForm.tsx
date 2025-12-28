import { useState } from "react";
import { X } from "lucide-react";
import { useAccountStore, Group } from "../../stores/accountStore";

interface Props {
  group: Group | null;
  onClose: () => void;
}

function GroupForm({ group, onClose }: Props) {
  const { createGroup, updateGroup, accounts } = useAccountStore();
  const isEditing = group !== null;

  const [formData, setFormData] = useState({
    name: group?.name ?? "",
    description: group?.description ?? "",
    account_ids: group?.account_ids ?? [],
    ranking: group?.ranking ?? 0,
  });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (isEditing && group) {
      await updateGroup({
        ...group,
        name: formData.name,
        description: formData.description || null,
        account_ids: formData.account_ids,
        ranking: formData.ranking,
      });
    } else {
      await createGroup({
        name: formData.name,
        description: formData.description || undefined,
        ranking: formData.ranking,
      });
    }

    onClose();
  };

  const toggleAccount = (accountId: string) => {
    setFormData((prev) => ({
      ...prev,
      account_ids: prev.account_ids.includes(accountId)
        ? prev.account_ids.filter((id) => id !== accountId)
        : [...prev.account_ids, accountId],
    }));
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-[60]">
      <div className="bg-[var(--color-bg-secondary)] rounded-lg w-[400px] max-h-[80vh] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
          <h3 className="font-semibold">
            {isEditing ? "Edit Group" : "Add Group"}
          </h3>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-4 space-y-4">
          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Group Name
            </label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) =>
                setFormData({ ...formData, name: e.target.value })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              required
            />
          </div>

          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Description (optional)
            </label>
            <input
              type="text"
              value={formData.description}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
            />
          </div>

          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Ranking
            </label>
            <input
              type="number"
              value={formData.ranking}
              onChange={(e) =>
                setFormData({ ...formData, ranking: parseInt(e.target.value) || 0 })
              }
              className="w-full px-3 py-2 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
              min={0}
              title="Lower ranking values appear first in the list."
            />
          </div>

          {isEditing && accounts.length > 0 && (
            <div>
              <label className="block text-sm text-[var(--color-text-secondary)] mb-2">
                Accounts
              </label>
              <div className="space-y-1 max-h-48 overflow-y-auto">
                {accounts.map((account) => (
                  <label
                    key={account.id}
                    className="flex items-center gap-2 p-2 bg-[var(--color-bg-tertiary)] rounded cursor-pointer hover:bg-[var(--color-border)] transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={formData.account_ids.includes(account.id)}
                      onChange={() => toggleAccount(account.id)}
                      className="rounded"
                    />
                    <span className="text-sm">
                      {account.role_name} (Server {account.server_id})
                    </span>
                  </label>
                ))}
              </div>
            </div>
          )}

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

export default GroupForm;

