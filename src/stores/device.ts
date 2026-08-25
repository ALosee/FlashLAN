import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface Device {
  id: string
  name: string
  ip: string
  platform: string
  port: number
  online?: boolean
}

export const useDeviceStore = defineStore('device', () => {
  const devices = ref<Device[]>([])
  const isDiscovering = ref(false)
  const localDevice = ref<Device | null>(null)
  const error = ref<string | null>(null)

  async function fetchLocal() {
    try {
      const info = await invoke<Device>('get_device_info')
      localDevice.value = { ...info, online: true }
    } catch (e) {
      error.value = String(e)
    }
  }

  async function discover() {
    isDiscovering.value = true
    error.value = null
    try {
      const result = await invoke<Device[]>('discover_devices')
      devices.value = result.map(d => ({ ...d, online: true }))
    } catch (e) {
      error.value = String(e)
    } finally {
      isDiscovering.value = false
    }
  }

  return { devices, isDiscovering, localDevice, error, fetchLocal, discover }
})
