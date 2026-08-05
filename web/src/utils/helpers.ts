export function debounce<Args extends unknown[]>(fn: (...args: Args) => void, ms = 300): (...args: Args) => void {
  let timeoutId: ReturnType<typeof setTimeout>;
  return function (this: unknown, ...args: Args) {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn.apply(this, args), ms);
  };
}

export function throttle<Args extends unknown[]>(fn: (...args: Args) => void, ms = 300): (...args: Args) => void {
  let lastCall = 0;
  return function (this: unknown, ...args: Args) {
    const now = Date.now();
    if (now - lastCall >= ms) {
      lastCall = now;
      fn.apply(this, args);
    }
  };
}

let idCounter = 0;
export function generateId(): string {
  return `id_${Date.now()}_${++idCounter}`;
}

export async function copyToClipboard(text: string | null | undefined): Promise<boolean> {
  if (!text) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function parseQueryParams(search: string | null | undefined): Record<string, string> {
  if (!search) return {};
  const params = new URLSearchParams(search);
  const result: Record<string, string> = {};
  for (const [key, value] of params.entries()) {
    result[key] = value;
  }
  return result;
}
