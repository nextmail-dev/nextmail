import { accountsApi } from "./api/accounts";
import { composerApi } from "./api/composer";
import { contactsApi } from "./api/contacts";
import { lifecycleApi } from "./api/lifecycle";
import { mailApi } from "./api/mail";
import { preferencesApi } from "./api/preferences";

export { normalizeCommandError } from "./commandErrors";

// Keep one stable public API surface for components while implementation stays
// grouped by business domain under src/app/api/.
export const api = {
  ...lifecycleApi,
  ...preferencesApi,
  ...accountsApi,
  ...mailApi,
  ...contactsApi,
  ...composerApi,
};
