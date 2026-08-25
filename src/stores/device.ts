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

export const useDeviceStore = defineStore('device', () => {
  const devices = ref<Device[]>([])
  const isDiscovering = ref(false)
  const localDevice = ref<Device | null>(null)
  const error = ref<string | null>(null)

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
      devices.value = [
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
      isDiscovering.value = false
      return
    }
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
