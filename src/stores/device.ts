import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Device {
  id: string
  name: string
  ip: string
  platform: string
  online: boolean
}

export const useDeviceStore = defineStore('device', () => {
  const devices = ref<Device[]>([])
  const isDiscovering = ref(false)

  async function discover() {
    isDiscovering.value = true
    // TODO: invoke('discover_devices')
    setTimeout(() => {
      devices.value = [
        { id: '1', name: 'MacBook Air', ip: '192.168.1.102', platform: 'macos', online: true },
        { id: '2', name: 'Windows PC', ip: '192.168.1.105', platform: 'windows', online: true },
      ]
      isDiscovering.value = false
    }, 800)
  }

  return { devices, isDiscovering, discover }
})
