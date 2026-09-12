import accounts from "./accounts.json";
import composer from "./composer.json";
import contacts from "./contacts.json";
import mail from "./mail.json";
import metadata from "./metadata.json";
import system from "./system.json";

export default {
  errors: {
    ...accounts,
    ...mail,
    ...composer,
    ...contacts,
    ...system,
  },
  ...metadata,
};
