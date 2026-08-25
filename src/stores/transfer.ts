import { defineStore } from 'pinia';
import { ref } from 'vue';

export interface TransferTask {
  id: string;
  fileName: string;
  size: number;
  progress: number;
  speed: number;
  status: 'pending' | 'transferring' | 'completed' | 'failed';
  targetDevice: string;
}

export const useTransferStore = defineStore('transfer', () => {
  const tasks = ref<TransferTask[]>([]);

  function addTask(task: TransferTask) {
    tasks.value.unshift(task);
  }

  return { tasks, addTask };
});
