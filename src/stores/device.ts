import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@/utils/tauri'

export interface Device {
  id: string
  name: string
  ip: string
  platform: string
  port: number
  online?: boolean
}

const MANUAL_DEVICES_STORAGE_KEY = 'flashlan.manual-devices'

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

    return stored.filter(
      (device): device is Device =>
        typeof device === 'object' &&
        device !== null &&
        typeof device.id === 'string' &&
        typeof device.name === 'string' &&
        typeof device.ip === 'string' &&
        typeof device.platform === 'string' &&
        typeof device.port === 'number',
    )
  } catch {
    return []
  }
}

export const useDeviceStore = defineStore('device', () => {
  const devices = ref<Device[]>([])
  const discoveredDevices = ref<Device[]>([])
  const manualDevices = ref<Device[]>(loadManualDevices())
  const isDiscovering = ref(false)
  const localDevice = ref<Device | null>(null)
  const error = ref<string | null>(null)

  function rebuildDevices() {
    const discoveredEndpoints = new Set(
      discoveredDevices.value.map(device => endpointKey(device.ip, device.port)),
    )
    devices.value = [
      ...discoveredDevices.value,
      ...manualDevices.value.filter(
        device => !discoveredEndpoints.has(endpointKey(device.ip, device.port)),
      ),
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

  function addManualDevice(targetIp: string, targetPort = 17321) {
    const ip = targetIp.trim()
    const device: Device = {
      id: `manual-${endpointKey(ip, targetPort)}`,
      name: ip,
      ip,
      platform: 'manual',
      port: targetPort,
      online: true,
    }
    const existingIndex = manualDevices.value.findIndex(
      item => endpointKey(item.ip, item.port) === endpointKey(ip, targetPort),
    )

    if (existingIndex === -1) {
      manualDevices.value.push(device)
    } else {
      manualDevices.value[existingIndex] = device
    }
    persistManualDevices()
    rebuildDevices()
    return device
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
    refreshManualStatus,
  }
})
