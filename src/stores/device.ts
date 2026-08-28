import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@/utils/tauri'

export interface Device {
  id: string
  name: string
  /** Optional user-defined display name. */
  alias?: string
  ip: string
  platform: string
  port: number
  /** Fingerprint is present for devices added through QR pairing. */
  fingerprint?: string
  /** Whether this device is saved locally, either manually or by pairing. */
  isManual?: boolean
  /** Whether the peer fingerprint is currently in the local trust store. */
  trusted?: boolean
  online?: boolean
}

const MANUAL_DEVICES_STORAGE_KEY = 'flashlan.manual-devices'
const DEVICE_ALIASES_STORAGE_KEY = 'flashlan.device-aliases'

function endpointKey(ip: string, port: number) {
  return `${ip}:${port}`
}

function loadManualDevices(): Device[] {
  if (typeof window === 'undefined') return []

  try {
    const stored = JSON.parse(
      window.localStorage.getItem(MANUAL_DEVICES_STORAGE_KEY) || '[]',
    ) as unknown
    if (!Array.isArray(stored)) return []

    return stored
      .filter(
        (device): device is Device =>
          typeof device === 'object' &&
          device !== null &&
          typeof device.id === 'string' &&
          typeof device.name === 'string' &&
          (typeof device.alias === 'undefined' || typeof device.alias === 'string') &&
          typeof device.ip === 'string' &&
          typeof device.platform === 'string' &&
          typeof device.port === 'number' &&
          (typeof device.fingerprint === 'undefined' || typeof device.fingerprint === 'string') &&
          (typeof device.trusted === 'undefined' || typeof device.trusted === 'boolean'),
      )
      .map(device => ({ ...device, isManual: true }))
  } catch {
    return []
  }
}

function loadDeviceAliases() {
  if (typeof window === 'undefined') return {}

  try {
    const stored = JSON.parse(
      window.localStorage.getItem(DEVICE_ALIASES_STORAGE_KEY) || '{}',
    ) as unknown
    if (typeof stored !== 'object' || stored === null || Array.isArray(stored)) return {}

    const aliases: Record<string, string> = {}
    Object.entries(stored).forEach(([key, value]) => {
      if (typeof value === 'string' && value.trim()) aliases[key] = value.trim().slice(0, 32)
    })
    return aliases
  } catch {
    return {}
  }
}

export const useDeviceStore = defineStore('device', () => {
  const devices = ref<Device[]>([])
  const discoveredDevices = ref<Device[]>([])
  const manualDevices = ref<Device[]>(loadManualDevices())
  const deviceAliases = ref<Record<string, string>>(loadDeviceAliases())
  const isDiscovering = ref(false)
  const localDevice = ref<Device | null>(null)
  const error = ref<string | null>(null)

  function applyDeviceAlias(device: Device): Device {
    const nextDevice = { ...device }
    const alias = deviceAliases.value[endpointKey(device.ip, device.port)]
    if (alias) nextDevice.alias = alias
    else delete nextDevice.alias
    return nextDevice
  }

  function rebuildDevices() {
    const manualByEndpoint = new Map(
      manualDevices.value.map(device => [endpointKey(device.ip, device.port), device]),
    )
    const discoveredEndpoints = new Set<string>()
    devices.value = [
      ...discoveredDevices.value.map(device => {
        const key = endpointKey(device.ip, device.port)
        const manual = manualByEndpoint.get(key)
        discoveredEndpoints.add(key)
        const nextDevice = applyDeviceAlias(device)
        return manual
          ? {
              ...nextDevice,
              isManual: true,
              fingerprint: manual.fingerprint,
              trusted: manual.trusted,
            }
          : nextDevice
      }),
      ...manualDevices.value
        .filter(device => !discoveredEndpoints.has(endpointKey(device.ip, device.port)))
        .map(applyDeviceAlias),
    ]
  }

  function persistManualDevices() {
    if (typeof window === 'undefined') return
    try {
      window.localStorage.setItem(MANUAL_DEVICES_STORAGE_KEY, JSON.stringify(manualDevices.value))
    } catch {
      // Local storage may be unavailable in a restricted webview.
    }
  }

  function persistDeviceAliases() {
    if (typeof window === 'undefined') return
    try {
      window.localStorage.setItem(DEVICE_ALIASES_STORAGE_KEY, JSON.stringify(deviceAliases.value))
    } catch {
      // Local storage may be unavailable in a restricted webview.
    }
  }

  async function fetchLocal() {
    if (!isTauri()) {
      localDevice.value = {
        id: 'browser-mock',
        name: 'Browser Preview',
        ip: '127.0.0.1',
        platform: 'browser',
        port: 17321,
        online: true,
      }
      return
    }
    try {
      const info = await invoke<Device>('get_device_info')
      localDevice.value = { ...info, online: true }
    } catch (e) {
      error.value = String(e)
    }
  }

  async function discover() {
    if (isDiscovering.value) return
    if (!isTauri()) {
      isDiscovering.value = true
      await new Promise(r => setTimeout(r, 600))
      discoveredDevices.value = [
        {
          id: 'mock-1',
          name: 'MacBook Air (Mock)',
          ip: '192.168.1.102',
          platform: 'macos',
          port: 17321,
          online: true,
        },
        {
          id: 'mock-2',
          name: 'Windows PC (Mock)',
          ip: '192.168.1.105',
          platform: 'windows',
          port: 17321,
          online: true,
        },
      ]
      rebuildDevices()
      isDiscovering.value = false
      return
    }
    isDiscovering.value = true
    error.value = null
    try {
      const result = await invoke<Device[]>('discover_devices')
      discoveredDevices.value = result.map(d => ({ ...d, online: true }))
      rebuildDevices()
    } catch (e) {
      error.value = String(e)
    } finally {
      isDiscovering.value = false
    }
  }

  async function testConnection(targetIp: string, targetPort = 17321) {
    if (!isTauri()) {
      await new Promise(resolve => setTimeout(resolve, 450))
      return
    }

    await invoke<void>('test_connection', {
      targetIp: targetIp.trim(),
      targetPort,
    })
  }

  /** Probe manual devices so the UI can show 离线 instead of a fake 在线. */
  async function refreshManualStatus() {
    if (!manualDevices.value.length) return
    if (!isTauri()) {
      manualDevices.value.forEach(device => {
        device.online = true
      })
      rebuildDevices()
      return
    }
    await Promise.allSettled(
      manualDevices.value.map(async device => {
        try {
          await invoke('test_connection', {
            targetIp: device.ip,
            targetPort: device.port,
          })
          device.online = true
        } catch {
          device.online = false
        }
      }),
    )
    rebuildDevices()
  }

  function addManualDevice(
    targetIp: string,
    targetPort = 17321,
    fingerprint?: string,
    trusted = false,
  ) {
    const ip = targetIp.trim()
    const normalizedFingerprint = fingerprint?.trim().toLowerCase() || undefined
    const existingIndex = manualDevices.value.findIndex(
      item => endpointKey(item.ip, item.port) === endpointKey(ip, targetPort),
    )
    const existingDevice = existingIndex === -1 ? undefined : manualDevices.value[existingIndex]
    const device: Device = {
      id: `manual-${endpointKey(ip, targetPort)}`,
      name: ip,
      ip,
      platform: 'manual',
      port: targetPort,
      online: true,
      isManual: true,
      trusted,
    }
    if (normalizedFingerprint || existingDevice?.fingerprint) {
      device.fingerprint = normalizedFingerprint || existingDevice?.fingerprint
    }
    if (!normalizedFingerprint && existingDevice) {
      device.trusted = existingDevice.trusted ?? false
    }

    if (existingIndex === -1) {
      manualDevices.value.push(device)
    } else {
      manualDevices.value[existingIndex] = device
    }
    persistManualDevices()
    rebuildDevices()
    return device
  }

  function removeManualDevice(deviceIdOrIp: string, targetPort?: number) {
    const nextDevices = manualDevices.value.filter(device => {
      if (targetPort !== undefined) {
        return endpointKey(device.ip, device.port) !== endpointKey(deviceIdOrIp, targetPort)
      }
      return device.id !== deviceIdOrIp
    })
    if (nextDevices.length === manualDevices.value.length) return false
    manualDevices.value
      .filter(device => !nextDevices.some(next => next.id === device.id))
      .forEach(device => {
        delete deviceAliases.value[endpointKey(device.ip, device.port)]
      })
    manualDevices.value = nextDevices
    persistManualDevices()
    persistDeviceAliases()
    rebuildDevices()
    return true
  }

  function setDeviceAlias(targetIp: string, targetPort: number, alias: string) {
    const key = endpointKey(targetIp.trim(), targetPort)
    const normalizedAlias = alias.trim().slice(0, 32)
    if (normalizedAlias) deviceAliases.value[key] = normalizedAlias
    else delete deviceAliases.value[key]
    persistDeviceAliases()
    rebuildDevices()
    return normalizedAlias
  }

  function setManualDeviceTrusted(targetIp: string, targetPort: number, trusted: boolean) {
    const device = manualDevices.value.find(
      item => endpointKey(item.ip, item.port) === endpointKey(targetIp, targetPort),
    )
    if (!device || !device.fingerprint) return false
    device.trusted = trusted
    persistManualDevices()
    rebuildDevices()
    return true
  }

  function syncTrustedDevices(fingerprints: string[]) {
    const trustedFingerprints = new Set(
      fingerprints.map(fingerprint => fingerprint.trim().toLowerCase()),
    )
    let changed = false
    manualDevices.value.forEach(device => {
      const trusted = Boolean(device.fingerprint && trustedFingerprints.has(device.fingerprint))
      if (device.trusted !== trusted) {
        device.trusted = trusted
        changed = true
      }
    })
    if (changed) persistManualDevices()
    rebuildDevices()
  }

  return {
    devices,
    isDiscovering,
    localDevice,
    error,
    fetchLocal,
    discover,
    testConnection,
    addManualDevice,
    removeManualDevice,
    setDeviceAlias,
    setManualDeviceTrusted,
    syncTrustedDevices,
    refreshManualStatus,
  }
})

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useDeviceStore, import.meta.hot))
}
