import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isTauri } from '@/utils/tauri'

export interface TransferTask {
  id: string
  fileName: string
  filePath?: string
  size?: number
  progress: number
  speed: number
  transferred: number
  total: number
  status: 'pending' | 'transferring' | 'completed' | 'failed'
  targetDevice: string
  targetIp: string
  error?: string
}

interface ProgressPayload {
  task_id: string
  file_name: string
  progress: number
  speed: number
  transferred: number
  total: number
}

interface CompletePayload {
  task_id: string
  file_name: string
  path: string
  success: boolean
  message: string
}

export const useTransferStore = defineStore('transfer', () => {
  const tasks = ref<TransferTask[]>([])
  const isListening = ref(false)

  function addTask(task: TransferTask) {
    tasks.value.unshift(task)
  }

  function updateProgress(payload: ProgressPayload) {
    const task = tasks.value.find(t => t.id === payload.task_id)
    if (task) {
      task.progress = payload.progress
      task.speed = payload.speed
      task.transferred = payload.transferred
      task.total = payload.total
      task.status = payload.progress >= 100 ? 'completed' : 'transferring'
    }
  }

  async function ensureListener() {
    if (isListening.value) return
    if (!isTauri()) return
    isListening.value = true
    try {
      await listen<ProgressPayload>('transfer_progress', event => {
        updateProgress(event.payload)
      })
      await listen<CompletePayload>('transfer_complete', event => {
        const task = tasks.value.find(t => t.id === event.payload.task_id)
        if (task) {
          task.status = event.payload.success ? 'completed' : 'failed'
          task.progress = 100
        }
      })
    } catch {
      isListening.value = false
    }
  }

  async function sendFile(filePath: string, targetIp: string, targetPort?: number) {
    if (!isTauri()) {
      const mockId = Math.random().toString(36).slice(2)
      const fileName = filePath.split('/').pop() || 'file'
      const task: TransferTask = {
        id: mockId,
        fileName,
        filePath,
        progress: 0,
        speed: 0,
        transferred: 0,
        total: 100,
        status: 'transferring',
        targetDevice: targetIp,
        targetIp,
      }
      tasks.value.unshift(task)
      let p = 0
      const timer = setInterval(() => {
        p += 20
        task.progress = Math.min(p, 100)
        task.transferred = task.progress
        task.total = 100
        task.speed = 1024 * 1024 * 2
        if (p >= 100) {
          task.status = 'completed'
          clearInterval(timer)
        }
      }, 300)
      return mockId
    }
    await ensureListener()
    const fileName = filePath.split('/').pop() || filePath.split('\\').pop() || 'file'
    const taskId = globalThis.crypto?.randomUUID?.() || Math.random().toString(36).slice(2)
    const task: TransferTask = {
      id: taskId,
      fileName,
      filePath,
      progress: 0,
      speed: 0,
      transferred: 0,
      total: 0,
      status: 'transferring',
      targetDevice: targetIp,
      targetIp,
    }
    tasks.value.unshift(task)
    try {
      const returnedId = await invoke<string>('send_file', {
        path: filePath,
        targetIp,
        targetPort,
        taskId,
      })
      // Use returned or keep generated
      task.id = returnedId || taskId
      return task.id
    } catch (e) {
      const msg = String(e)
      console.error('[FlashLAN] send_file failed', { filePath, targetIp, error: msg })
      task.status = 'failed'
      task.error = msg
      throw e
    }
  }

  return { tasks, addTask, updateProgress, ensureListener, sendFile }
})
