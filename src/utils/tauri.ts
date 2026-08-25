export function isTauri(): boolean {
  if (typeof window === 'undefined') return false
  const w = window as unknown as Record<string, unknown>
  return '__TAURI__' in w || '__TAURI_INTERNALS__' in w || '__TAURI_IPC__' in w
}

export async function safeInvoke<T>(fn: () => Promise<T>, fallback: T): Promise<T> {
  if (!isTauri()) return fallback
  try {
    return await fn()
  } catch {
    return fallback
  }
}
