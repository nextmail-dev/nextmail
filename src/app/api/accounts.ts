import type {
  AccountConnectionDraft,
  AccountDraft,
  AccountManagementDetail,
  AccountRemovalImpact,
  AccountRuntimeSummary,
  AccountSummary,
  ConnectionTestResult,
  DiscoveredAccountConfig,
  SyncInterval,
} from "../types";
import { invoke } from "./invoke";

export const accountsApi = {
  discoverAccountConfig: (email: string) =>
    invoke<DiscoveredAccountConfig>("discover_account_config", { email }),
  testAccountConnections: (draft: AccountDraft) =>
    invoke<ConnectionTestResult>("test_account_connections", { draft }),
  savePasswordAccount: (draft: AccountDraft) =>
    invoke<AccountSummary>("save_password_account", { draft }),
  addPasswordAccount: (draft: AccountDraft) =>
    invoke<AccountSummary>("add_password_account", { draft }),
  listAccountSummaries: () => invoke<AccountSummary[]>("list_account_summaries"),
  getAccountConnectionDraft: (accountId: string) =>
    invoke<AccountConnectionDraft>("get_account_connection_draft", { accountId }),
  updatePasswordAccount: (
    accountId: string,
    draft: AccountConnectionDraft,
    newPassword: string | null,
  ) => invoke<AccountSummary>("update_password_account", { accountId, draft, newPassword }),
  reauthenticatePasswordAccount: (accountId: string, password: string) =>
    invoke<AccountSummary>("reauthenticate_password_account", { accountId, password }),
  getAccountRemovalImpact: (accountId: string) =>
    invoke<AccountRemovalImpact>("get_account_removal_impact", { accountId }),
  removeAccount: (accountId: string) => invoke<void>("remove_account", { accountId }),
  listAccountRuntimeSummaries: () =>
    invoke<AccountRuntimeSummary[]>("list_account_runtime_summaries"),
  getLastSelectedAccount: () => invoke<string | null>("get_last_selected_account"),
  setLastSelectedAccount: (accountId: string) =>
    invoke<string>("set_last_selected_account", { accountId }),
  setLastSelectedMailbox: (accountId: string, mailboxId: string) =>
    invoke<string>("set_last_selected_mailbox", { accountId, mailboxId }),
  getAccountManagementDetail: (accountId: string) =>
    invoke<AccountManagementDetail>("get_account_management_detail", { accountId }),
  setAccountSyncInterval: (accountId: string, syncInterval: SyncInterval) =>
    invoke<SyncInterval>("set_account_sync_interval", { accountId, syncInterval }),
  setAccountDownloadFullMessages: (accountId: string, enabled: boolean) =>
    invoke<boolean>("set_account_download_full_messages", { accountId, enabled }),
};
