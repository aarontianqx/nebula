import { useState, useEffect } from "react";
import { X, Users, User } from "lucide-react";
import AccountForm from "../forms/AccountForm";
import GroupForm from "../forms/GroupForm";
import { useAccountStore, Account, Group } from "../../stores/accountStore";

interface Props {
  onClose: () => void;
}

type Tab = "accounts" | "groups";

function ManagementDialog({ onClose }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>("accounts");
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  const [editingGroup, setEditingGroup] = useState<Group | null>(null);
  const [showAccountForm, setShowAccountForm] = useState(false);
  const [showGroupForm, setShowGroupForm] = useState(false);

  const { accounts, groups, deleteAccount, deleteGroup, fetchAccounts, fetchGroups } = useAccountStore();

  // Refresh data when dialog opens (supports cross-instance sync)
  useEffect(() => {
    fetchAccounts();
    fetchGroups();
  }, [fetchAccounts, fetchGroups]);

  const handleEditAccount = (account: Account) => {
    setEditingAccount(account);
    setShowAccountForm(true);
  };

  const handleEditGroup = (group: Group) => {
    setEditingGroup(group);
    setShowGroupForm(true);
  };

  const handleCloseAccountForm = () => {
    setEditingAccount(null);
    setShowAccountForm(false);
  };

  const handleCloseGroupForm = () => {
    setEditingGroup(null);
    setShowGroupForm(false);
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-secondary)] rounded-lg w-[700px] max-h-[80vh] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
          <h2 className="text-lg font-semibold">Account Management</h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-[var(--color-border)]">
          <button
            onClick={() => setActiveTab("accounts")}
            className={`flex items-center gap-2 px-4 py-2 text-sm font-medium transition-colors ${activeTab === "accounts"
                ? "text-[var(--color-accent)] border-b-2 border-[var(--color-accent)]"
                : "text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
              }`}
          >
            <User size={16} />
            Accounts ({accounts.length})
          </button>
          <button
            onClick={() => setActiveTab("groups")}
            className={`flex items-center gap-2 px-4 py-2 text-sm font-medium transition-colors ${activeTab === "groups"
                ? "text-[var(--color-accent)] border-b-2 border-[var(--color-accent)]"
                : "text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
              }`}
          >
            <Users size={16} />
            Groups ({groups.length})
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {activeTab === "accounts" && (
            <div>
              <button
                onClick={() => setShowAccountForm(true)}
                className="mb-4 px-4 py-2 bg-[var(--color-accent)] text-white rounded-md hover:bg-[var(--color-accent-hover)] transition-colors"
              >
                Add Account
              </button>

              {accounts.length === 0 ? (
                <p className="text-[var(--color-text-muted)] text-center py-8">
                  No accounts yet
                </p>
              ) : (
                <div className="space-y-2">
                  {accounts.map((account) => (
                    <div
                      key={account.id}
                      className="flex items-center justify-between p-3 bg-[var(--color-bg-tertiary)] rounded-md"
                    >
                      <div>
                        <div className="font-medium">{account.role_name}</div>
                        <div className="text-sm text-[var(--color-text-muted)]">
                          {account.user_name} · Server {account.server_id}
                        </div>
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditAccount(account)}
                          className="px-3 py-1 text-sm bg-[var(--color-bg-secondary)] rounded hover:bg-[var(--color-border)] transition-colors"
                        >
                          Edit
                        </button>
                        <button
                          onClick={() => deleteAccount(account.id)}
                          className="px-3 py-1 text-sm bg-[var(--color-error)] text-white rounded hover:opacity-80 transition-opacity"
                        >
                          Delete
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {activeTab === "groups" && (
            <div>
              <button
                onClick={() => setShowGroupForm(true)}
                className="mb-4 px-4 py-2 bg-[var(--color-accent)] text-white rounded-md hover:bg-[var(--color-accent-hover)] transition-colors"
              >
                Add Group
              </button>

              {groups.length === 0 ? (
                <p className="text-[var(--color-text-muted)] text-center py-8">
                  No groups yet
                </p>
              ) : (
                <div className="space-y-2">
                  {groups.map((group) => (
                    <div
                      key={group.id}
                      className="flex items-center justify-between p-3 bg-[var(--color-bg-tertiary)] rounded-md"
                    >
                      <div>
                        <div className="font-medium">{group.name}</div>
                        <div className="text-sm text-[var(--color-text-muted)]">
                          {group.account_ids.length} accounts
                          {group.description && ` · ${group.description}`}
                        </div>
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditGroup(group)}
                          className="px-3 py-1 text-sm bg-[var(--color-bg-secondary)] rounded hover:bg-[var(--color-border)] transition-colors"
                        >
                          Edit
                        </button>
                        <button
                          onClick={() => deleteGroup(group.id)}
                          className="px-3 py-1 text-sm bg-[var(--color-error)] text-white rounded hover:opacity-80 transition-opacity"
                        >
                          Delete
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Account Form Dialog */}
      {showAccountForm && (
        <AccountForm account={editingAccount} onClose={handleCloseAccountForm} />
      )}

      {/* Group Form Dialog */}
      {showGroupForm && (
        <GroupForm group={editingGroup} onClose={handleCloseGroupForm} />
      )}
    </div>
  );
}

export default ManagementDialog;

