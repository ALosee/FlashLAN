export interface AppNavigationItem {
  label: string
  icon: string
  path: string
}

export type StatusTone =
  | 'neutral'
  | 'primary'
  | 'info'
  | 'success'
  | 'warning'
  | 'destructive'
