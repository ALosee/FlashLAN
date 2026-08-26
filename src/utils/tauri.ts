export function isTauri(): boolean {
  if (typeof window === 'undefined') return false
  const w = window as unknown as Record<string, unknown>
  return '__TAURI__' in w || '__TAURI_INTERNALS__' in w || '__TAURI_IPC__' in w
}

export function isMobilePlatform(): boolean {
  if (typeof navigator === 'undefined') return false

  const userAgent = navigator.userAgent
  return (
    /Android|iPhone|iPad|iPod/i.test(userAgent) ||
    (/Macintosh/i.test(userAgent) && navigator.maxTouchPoints > 1)
  )
}

export async function safeInvoke<T>(fn: () => Promise<T>, fallback: T): Promise<T> {
  if (!isTauri()) return fallback
  try {
    return await fn()
  } catch {
    return fallback
  }
}
