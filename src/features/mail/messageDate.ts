export function formatMessageListTimestamp(
  receivedAtSeconds: number,
  yesterdayLabel: string,
  now = new Date(),
) {
  const receivedAt = new Date(receivedAtSeconds * 1000);
  if (Number.isNaN(receivedAt.getTime())) return "";

  if (isSameLocalDate(receivedAt, now)) {
    return `${pad(receivedAt.getHours())}:${pad(receivedAt.getMinutes())}`;
  }

  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (isSameLocalDate(receivedAt, yesterday)) return yesterdayLabel;

  const monthAndDay = `${pad(receivedAt.getMonth() + 1)}-${pad(receivedAt.getDate())}`;
  return receivedAt.getFullYear() === now.getFullYear()
    ? monthAndDay
    : `${receivedAt.getFullYear()}-${monthAndDay}`;
}

function isSameLocalDate(left: Date, right: Date) {
  return left.getFullYear() === right.getFullYear()
    && left.getMonth() === right.getMonth()
    && left.getDate() === right.getDate();
}

function pad(value: number) {
  return String(value).padStart(2, "0");
}
