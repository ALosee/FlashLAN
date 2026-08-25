export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window
}

export async function safeInvoke<T>(fn: () => Promise<T>, fallback: T): Promise<T> {
  if (!isTauri()) return fallback
  try {
    return await fn()
  } catch {
    return fallback
  }
}
