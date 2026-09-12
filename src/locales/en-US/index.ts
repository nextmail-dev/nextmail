import accounts from "./accounts.json";
import common from "./common.json";
import composer from "./composer.json";
import contacts from "./contacts.json";
import errors from "./errors";
import mail from "./mail.json";
import onboarding from "./onboarding.json";
import settings from "./settings.json";

export default {
  ...common,
  ...onboarding,
  ...mail,
  ...contacts,
  ...composer,
  ...accounts,
  ...settings,
  ...errors,
};
