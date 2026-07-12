import type { MessageAddress } from "@/app/types";

export function parseAddresses(value: string): MessageAddress[] {
  return value
    .split(/[;,]/)
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => {
      const match = item.match(/^(.*?)\s*<([^<>]+)>$/);
      return match
        ? { name: match[1].trim() || null, email: match[2].trim() }
        : { name: null, email: item };
    });
}

export function formatAddresses(values: MessageAddress[]) {
  return values
    .map((value) => (value.name ? `${value.name} <${value.email}>` : value.email))
    .join(", ");
}
