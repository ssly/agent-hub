import { ref } from 'vue'

// Two-step confirm that disarms the moment the pointer leaves the trigger
// element. Replaces the old click-to-confirm + setTimeout-reset pattern:
// the confirmation now lives only while the cursor stays over the button, so
// users cancel by simply moving away instead of waiting out a timer.
//
// Two flavors mirror the two state shapes used across the app:
//   - id-keyed (one armed item per list, e.g. per-row delete)
//   - boolean  (a single button, e.g. "empty trash")

export function useHoverResetId() {
  const armedId = ref<string | null>(null)
  function arm(id: string) { armedId.value = id }
  function reset() { armedId.value = null }
  return { armedId, arm, reset }
}

export function useHoverResetBool() {
  const armed = ref(false)
  function arm() { armed.value = true }
  function reset() { armed.value = false }
  return { armed, arm, reset }
}
