import { useEffect, useState } from "react";
import { Settings } from "lucide-react";
import { useAccountStore } from "../../stores/accountStore";
import ManagementDialog from "../dialogs/ManagementDialog";

function MainWindow() {
  const { accounts, groups, fetchAccounts, fetchGroups } = useAccountStore();
  const [showManagement, setShowManagement] = useState(false);

  useEffect(() => {
    fetchAccounts();
    fetchGroups();
  }, [fetchAccounts, fetchGroups]);

  return (
    <div className="flex flex-col h-screen bg-[var(--color-bg-primary)]">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-3 bg-[var(--color-bg-secondary)] border-b border-[var(--color-border)]">
        <div className="flex items-center gap-4">
          <h1 className="text-lg font-semibold text-[var(--color-text-primary)]">
            Wardenly
          </h1>
          <span className="text-sm text-[var(--color-text-secondary)]">
            {accounts.length} accounts · {groups.length} groups
          </span>
        </div>
        <button
          onClick={() => setShowManagement(true)}
          className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] hover:bg-[var(--color-border)] transition-colors"
        >
          <Settings size={16} />
          Manage
        </button>
      </div>

      {/* Main Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Account List */}
        <div className="w-64 flex-shrink-0 bg-[var(--color-bg-secondary)] border-r border-[var(--color-border)] overflow-y-auto">
          <div className="p-2">
            {accounts.length === 0 ? (
              <div className="p-4 text-center text-[var(--color-text-muted)]">
                No accounts yet.
                <br />
                <button
                  onClick={() => setShowManagement(true)}
                  className="mt-2 text-[var(--color-accent)] hover:underline"
                >
                  Add one
                </button>
              </div>
            ) : (
              accounts.map((account) => (
                <div
                  key={account.id}
                  className="p-3 mb-1 rounded-md bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-border)] cursor-pointer transition-colors"
                >
                  <div className="text-sm font-medium text-[var(--color-text-primary)]">
                    {account.role_name}
                  </div>
                  <div className="text-xs text-[var(--color-text-muted)]">
                    Server {account.server_id}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Detail Panel */}
        <div className="flex-1 flex items-center justify-center text-[var(--color-text-muted)]">
          <div className="text-center">
            <p className="text-lg">Select an account to start</p>
            <p className="text-sm mt-2">
              Or use the Manage button to add accounts
            </p>
          </div>
        </div>
      </div>

      {/* Management Dialog */}
      {showManagement && (
        <ManagementDialog onClose={() => setShowManagement(false)} />
      )}
    </div>
  );
}

export default MainWindow;

