import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isTauri } from '@/utils/tauri'

export interface TransferTask {
  id: string
  fileName: string
  filePath?: string
  fileOpenPath?: string
  size?: number
  progress: number
  speed: number
  transferred: number
  total: number
  status: 'pending' | 'transferring' | 'completed' | 'failed'
  direction: 'send' | 'receive'
  targetDevice: string
  targetIp: string
  error?: string
}

interface StartedPayload {
  task_id: string
  file_name: string
  total: number
  direction: 'send' | 'receive'
  peer: string
  path: string
}

export interface TransferRequest {
  taskId: string
  fileName: string
  total: number
  peer: string
}

interface RequestPayload {
  task_id: string
  file_name: string
  total: number
  peer: string
}

function toTransferRequest(payload: RequestPayload): TransferRequest {
  return {
    taskId: payload.task_id,
    fileName: payload.file_name,
    total: payload.total,
    peer: payload.peer,
  }
}

interface ProgressPayload {
  task_id: string
  file_name: string
  progress: number
  speed: number
  transferred: number
  total: number
  direction?: 'send' | 'receive'
  peer?: string
}

interface CompletePayload {
  task_id: string
  file_name: string
  path: string
  open_path?: string
  success: boolean
  message: string
  direction?: 'send' | 'receive'
  peer?: string
}

export const useTransferStore = defineStore('transfer', () => {
  const historyStorageKey = 'flashlan.transfer-history'
  const autoReceiveStorageKey = 'flashlan.auto-receive'

  function loadTasks() {
    if (typeof localStorage === 'undefined') return [] as TransferTask[]
    try {
      const value = localStorage.getItem(historyStorageKey)
      const parsed = value ? JSON.parse(value) : []
      return Array.isArray(parsed) ? (parsed as TransferTask[]) : []
    } catch {
      return [] as TransferTask[]
    }
  }

  const tasks = ref<TransferTask[]>(loadTasks())
  const pendingRequests = ref<TransferRequest[]>([])
  const autoReceiveEnabled = ref(
    typeof localStorage !== 'undefined' && localStorage.getItem(autoReceiveStorageKey) === 'true',
  )
  const isListening = ref(false)

  watch(
    tasks,
    value => {
      if (typeof localStorage === 'undefined') return
      const history = value
        .filter(task => task.status === 'completed' || task.status === 'failed')
        .slice(0, 100)
      localStorage.setItem(historyStorageKey, JSON.stringify(history))
    },
    { deep: true },
  )

  function addTask(task: TransferTask) {
    tasks.value.unshift(task)
  }

  function removeTask(taskId: string) {
    tasks.value = tasks.value.filter(task => task.id !== taskId)
  }

  function clearHistory() {
    tasks.value = tasks.value.filter(
      task => task.status === 'transferring' || task.status === 'pending',
    )
  }

  function ensureTask(payload: {
    taskId: string
    fileName: string
    total: number
    direction: 'send' | 'receive'
    peer: string
    path?: string
  }) {
    const existing = tasks.value.find(task => task.id === payload.taskId)
    if (existing) return existing

    const task: TransferTask = {
      id: payload.taskId,
      fileName: payload.fileName,
      filePath: payload.path,
      progress: 0,
      speed: 0,
      transferred: 0,
      total: payload.total,
      status: 'transferring',
      direction: payload.direction,
      targetDevice: payload.peer,
      targetIp: payload.peer,
    }
    tasks.value.unshift(task)
    return task
  }

  function handleStarted(payload: StartedPayload) {
    ensureTask({
      taskId: payload.task_id,
      fileName: payload.file_name,
      total: payload.total,
      direction: payload.direction,
      peer: payload.peer,
      path: payload.path,
    })
  }

  function updateProgress(payload: ProgressPayload) {
    const task =
      tasks.value.find(t => t.id === payload.task_id) ||
      ensureTask({
        taskId: payload.task_id,
        fileName: payload.file_name,
        total: payload.total,
        direction: payload.direction || 'receive',
        peer: payload.peer || '局域网设备',
      })
    task.progress = Math.min(payload.progress, 100)
    task.speed = payload.speed
    task.transferred = payload.transferred
    task.total = payload.total
    task.status = payload.progress >= 100 ? 'completed' : 'transferring'
  }

  function handleTransferRequest(payload: RequestPayload) {
    const request = toTransferRequest(payload)
    if (autoReceiveEnabled.value) {
      void respondToRequest(request.taskId, true).catch(error => {
        console.error('[FlashLAN] auto receive response failed', error)
      })
      return
    }
    if (!pendingRequests.value.some(item => item.taskId === request.taskId)) {
      pendingRequests.value.push(request)
    }
  }

  async function respondToRequest(taskId: string, accepted: boolean) {
    const index = pendingRequests.value.findIndex(item => item.taskId === taskId)
    const request = index >= 0 ? pendingRequests.value.splice(index, 1)[0] : undefined
    if (!isTauri()) return
    try {
      await invoke('respond_transfer_request', { taskId, accepted })
    } catch (error) {
      if (request && !pendingRequests.value.some(item => item.taskId === taskId)) {
        pendingRequests.value.unshift(request)
      }
      throw error
    }
  }

  async function setAutoReceive(enabled: boolean) {
    autoReceiveEnabled.value = enabled
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(autoReceiveStorageKey, String(enabled))
    }
    if (isTauri()) {
      await invoke('set_auto_receive', { enabled })
    }
    if (enabled && pendingRequests.value.length > 0) {
      const requests = pendingRequests.value.map(request => request.taskId)
      await Promise.allSettled(requests.map(taskId => respondToRequest(taskId, true)))
    }
  }

  async function initialize() {
    await ensureListener()
    if (!isTauri()) return
    await invoke('set_auto_receive', { enabled: autoReceiveEnabled.value })
    const pending = await invoke<RequestPayload[]>('get_pending_transfer_requests')
    pending.forEach(handleTransferRequest)
  }

  async function ensureListener() {
    if (isListening.value) return
    if (!isTauri()) return
    isListening.value = true
    try {
      await listen<RequestPayload>('transfer_request', event => {
        handleTransferRequest(event.payload)
      })
      await listen<StartedPayload>('transfer_started', event => {
        handleStarted(event.payload)
      })
      await listen<ProgressPayload>('transfer_progress', event => {
        updateProgress(event.payload)
      })
      await listen<CompletePayload>('transfer_complete', event => {
        const task =
          tasks.value.find(t => t.id === event.payload.task_id) ||
          ensureTask({
            taskId: event.payload.task_id,
            fileName: event.payload.file_name,
            total: 0,
            direction: event.payload.direction || 'receive',
            peer: event.payload.peer || '局域网设备',
          })
        task.status = event.payload.success ? 'completed' : 'failed'
        task.progress = event.payload.success ? 100 : task.progress
        task.filePath = event.payload.path || task.filePath
        task.fileOpenPath = event.payload.open_path || task.fileOpenPath
        task.error = event.payload.success ? undefined : event.payload.message
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
        fileOpenPath: filePath,
        progress: 0,
        speed: 0,
        transferred: 0,
        total: 100,
        status: 'transferring',
        direction: 'send',
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
      fileOpenPath: filePath,
      progress: 0,
      speed: 0,
      transferred: 0,
      total: 0,
      status: 'transferring',
      direction: 'send',
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

  return {
    tasks,
    pendingRequests,
    autoReceiveEnabled,
    addTask,
    removeTask,
    clearHistory,
    updateProgress,
    ensureListener,
    initialize,
    respondToRequest,
    setAutoReceive,
    sendFile,
  }
})
