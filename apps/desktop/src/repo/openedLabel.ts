/** "2 min ago", "Yesterday", "Aug 12": how long ago a repository was opened. */
export function openedLabel(openedAt: number, now: Date = new Date()): string {
  const seconds = Math.max(0, Math.round(now.getTime() / 1000) - openedAt);
  if (seconds < 60) return "Just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return new Date(openedAt * 1000).toLocaleDateString("en", {
    month: "short",
    day: "numeric",
  });
}
