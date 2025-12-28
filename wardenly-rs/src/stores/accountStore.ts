import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface Account {
  id: string;
  role_name: string;
  user_name: string;
  password: string;
  server_id: number;
  ranking: number;
  cookies: string | null;
}

export interface Group {
  id: string;
  name: string;
  description: string | null;
  account_ids: string[];
  ranking: number;
}

interface AccountStore {
  accounts: Account[];
  groups: Group[];
  loading: boolean;
  error: string | null;

  // Account actions
  fetchAccounts: () => Promise<void>;
  createAccount: (data: Omit<Account, "id" | "cookies">) => Promise<void>;
  updateAccount: (account: Account) => Promise<void>;
  deleteAccount: (id: string) => Promise<void>;

  // Group actions
  fetchGroups: () => Promise<void>;
  createGroup: (data: { name: string; description?: string }) => Promise<void>;
  updateGroup: (group: Group) => Promise<void>;
  deleteGroup: (id: string) => Promise<void>;
}

export const useAccountStore = create<AccountStore>((set, get) => ({
  accounts: [],
  groups: [],
  loading: false,
  error: null,

  fetchAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const accounts = await invoke<Account[]>("get_accounts");
      set({ accounts, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  createAccount: async (data) => {
    set({ loading: true, error: null });
    try {
      await invoke("create_account", { request: data });
      await get().fetchAccounts();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  updateAccount: async (account) => {
    set({ loading: true, error: null });
    try {
      await invoke("update_account", { account });
      await get().fetchAccounts();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  deleteAccount: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("delete_account", { id });
      await get().fetchAccounts();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  fetchGroups: async () => {
    set({ loading: true, error: null });
    try {
      const groups = await invoke<Group[]>("get_groups");
      set({ groups, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  createGroup: async (data) => {
    set({ loading: true, error: null });
    try {
      await invoke("create_group", { request: data });
      await get().fetchGroups();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  updateGroup: async (group) => {
    set({ loading: true, error: null });
    try {
      await invoke("update_group", { group });
      await get().fetchGroups();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  deleteGroup: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("delete_group", { id });
      await get().fetchGroups();
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },
}));

