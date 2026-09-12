import type {
  AddressPresentation,
  ContactDetail,
  ContactDraft,
  ContactGroupDetail,
  ContactGroupDraft,
  ContactGroupSummary,
  ContactListPage,
  ContactSuggestions,
  ContactSummary,
  MessageAddress,
} from "../types";
import { invoke } from "./invoke";

export const contactsApi = {
  listContacts: (
    accountId: string,
    query: string,
    cursor: string | null,
    limit = 50,
  ) => invoke<ContactListPage>("list_contacts", { accountId, query, cursor, limit }),
  listContactSuggestions: (accountId: string, query: string, limit = 8) =>
    invoke<ContactSuggestions>("list_contact_suggestions", { accountId, query, limit }),
  listContactGroups: (accountId: string) =>
    invoke<ContactGroupSummary[]>("list_contact_groups", { accountId }),
  getContactGroup: (accountId: string, groupId: string) =>
    invoke<ContactGroupDetail>("get_contact_group", { accountId, groupId }),
  saveContactGroup: (
    accountId: string,
    groupId: string | null,
    draft: ContactGroupDraft,
    expectedRevision: number | null,
  ) => invoke<ContactGroupDetail>("save_contact_group", {
    accountId, groupId, draft, expectedRevision,
  }),
  deleteContactGroup: (accountId: string, groupId: string, expectedRevision: number) =>
    invoke<void>("delete_contact_group", { accountId, groupId, expectedRevision }),
  resolveContactAddresses: (accountId: string, addresses: MessageAddress[]) =>
    invoke<AddressPresentation[]>("resolve_contact_addresses", { accountId, addresses }),
  getContactDetail: (accountId: string, contactId: string) =>
    invoke<ContactDetail>("get_contact_detail", { accountId, contactId }),
  getContactSummary: (accountId: string, contactId: string) =>
    invoke<ContactSummary>("get_contact_summary", { accountId, contactId }),
  createContact: (accountId: string, draft: ContactDraft) =>
    invoke<ContactSummary>("create_contact", { accountId, draft }),
  updateContactName: (
    accountId: string,
    contactId: string,
    name: string,
    expectedRevision: number,
  ) => invoke<ContactSummary>("update_contact_name", {
    accountId, contactId, name, expectedRevision,
  }),
  deleteContacts: (accountId: string, contactIds: string[]) =>
    invoke<void>("delete_contacts", { accountId, contactIds }),
  openContactComposer: (accountId: string, contactId: string) =>
    invoke<string>("open_contact_composer", { accountId, contactId }),
};
