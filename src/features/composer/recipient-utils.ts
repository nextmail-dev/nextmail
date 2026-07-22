import type { MessageAddress } from "@/app/types";

const EMAIL_PATTERN = /^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*$/i;

export function isValidEmailAddress(value: string) {
  const email = value.trim();
  return email.length <= 254
    && !email.includes("..")
    && EMAIL_PATTERN.test(email);
}

export function parseAddress(value: string): MessageAddress | null {
  const item = value.trim();
  const match = item.match(/^(.*?)\s*<([^<>]+)>$/);
  const name = match?.[1].trim() || null;
  const email = (match?.[2] ?? item).trim();
  return isValidEmailAddress(email) ? { name, email } : null;
}

export function addRecipientInput(current: MessageAddress[], value: string) {
  const items = value.split(/[;,\n]+/).map((item) => item.trim()).filter(Boolean);
  if (!items.length) return { addresses: current, invalid: null };
  const parsed = items.map((item) => ({ item, address: parseAddress(item) }));
  const invalid = parsed.find((item) => !item.address)?.item ?? null;
  if (invalid) return { addresses: current, invalid };

  const addresses = [...current];
  for (const item of parsed) {
    const address = item.address!;
    if (!addresses.some((existing) => existing.email.toLocaleLowerCase() === address.email.toLocaleLowerCase())) {
      addresses.push(address);
    }
  }
  return { addresses, invalid: null };
}

export function parseAddresses(value: string): MessageAddress[] {
  return value
    .split(/[;,\n]/)
    .map((item) => item.trim())
    .filter(Boolean)
    .map(parseAddress)
    .filter((value): value is MessageAddress => value !== null);
}

export function formatAddresses(values: MessageAddress[]) {
  return values
    .map((value) => (value.name ? `${value.name} <${value.email}>` : value.email))
    .join(", ");
}
