<script lang="ts">
  import { X } from 'lucide-svelte';
  import { ui } from '$lib/stores/ui.svelte';
</script>

<div class="toast-region" aria-live="polite" aria-atomic="false">
  {#each ui.toasts as toast (toast.id)}
    <div class="toast {toast.kind}" role="status">
      <span class="message">{toast.message}</span>
      <button
        type="button"
        class="close"
        aria-label="Dismiss notification"
        onclick={() => ui.dismissToast(toast.id)}
      >
        <X size={14} strokeWidth={1.5} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-region {
    position: fixed;
    bottom: calc(var(--bv-actionbar-h) + var(--bv-space-3));
    right: var(--bv-space-4);
    display: flex;
    flex-direction: column-reverse;
    gap: var(--bv-space-2);
    z-index: 1000;
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--bv-space-3);
    padding: 10px 14px;
    border: 1px solid var(--bv-border);
    border-radius: 6px;
    background: var(--bv-surface);
    color: var(--bv-text);
    font-size: 13px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
    min-width: 220px;
    max-width: 360px;
    animation: slide-in var(--bv-dur-med) var(--bv-ease);
  }

  @keyframes slide-in {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .toast.success {
    border-color: color-mix(in srgb, var(--bv-accent) 30%, var(--bv-border));
  }
  .toast.warn {
    color: var(--bv-accent);
  }
  .toast.error {
    border-color: var(--bv-accent);
    color: var(--bv-accent);
  }

  .message {
    flex: 1;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    color: var(--bv-text-muted);
  }
  .close:hover {
    background: color-mix(in srgb, var(--bv-text) 8%, transparent);
    color: var(--bv-text);
  }
</style>
