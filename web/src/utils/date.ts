export type DateInput = string | number | Date;

export function formatDate(date: DateInput, options: Intl.DateTimeFormatOptions = {}): string {
  const d = new Date(date);
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    ...options
  };
  return d.toLocaleDateString('en-US', defaultOptions);
}

export function formatDateTime(date: DateInput): string {
  return formatDate(date, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
}

export function timeAgo(date: DateInput): string {
  const seconds = Math.floor((new Date().getTime() - new Date(date).getTime()) / 1000);

  const intervals: Record<string, number> = {
    year: 31536000,
    month: 2592000,
    week: 604800,
    day: 86400,
    hour: 3600,
    minute: 60
  };

  for (const [unit, secondsInUnit] of Object.entries(intervals)) {
    const interval = Math.floor(seconds / secondsInUnit);
    if (interval >= 1) {
      return `${interval} ${unit}${interval === 1 ? '' : 's'} ago`;
    }
  }
  return 'Just now';
}

export function formatRelativeTime(date: DateInput): string {
  return timeAgo(date);
}

export function isToday(date: DateInput): boolean {
  const d = new Date(date);
  const today = new Date();
  return d.toDateString() === today.toDateString();
}

export function formatShortDate(date: DateInput): string {
  return formatDate(date, { month: 'short', day: 'numeric' });
}
