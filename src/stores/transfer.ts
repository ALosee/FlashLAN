import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

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
    isListening.value = true
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
  }

  async function sendFile(filePath: string, targetIp: string, targetPort?: number) {
    await ensureListener()
    const fileName = filePath.split('/').pop() || filePath.split('\\').pop() || 'file'
    const task: TransferTask = {
      id: '',
      fileName,
      filePath,
      progress: 0,
      speed: 0,
      transferred: 0,
      total: 0,
      status: 'pending',
      targetDevice: targetIp,
      targetIp,
    }
    try {
      const taskId = await invoke<string>('send_file', {
        path: filePath,
        targetIp,
        targetPort,
      })
      task.id = taskId
      task.status = 'transferring'
      tasks.value.unshift(task)
      return taskId
    } catch (e) {
      task.status = 'failed'
      tasks.value.unshift(task)
      throw e
    }
  }

  return { tasks, addTask, updateProgress, ensureListener, sendFile }
})
