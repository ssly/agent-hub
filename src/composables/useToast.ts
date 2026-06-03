import { ref } from 'vue'

export type ToastType = 'info' | 'success' | 'warning' | 'error'

export interface ToastItem {
  id: number
  message: string
  type: ToastType
  exiting?: boolean
}

const toasts = ref<ToastItem[]>([])
let nextId = 0

export function useToast() {
  function showToast(message: string, type: ToastType = 'info', duration = 3000) {
    const id = nextId++
    toasts.value.push({ id, message, type })
    setTimeout(() => {
      const item = toasts.value.find(t => t.id === id)
      if (item) item.exiting = true
      setTimeout(() => {
        toasts.value = toasts.value.filter(t => t.id !== id)
      }, 200)
    }, duration)
  }

  return { toasts, showToast }
}
