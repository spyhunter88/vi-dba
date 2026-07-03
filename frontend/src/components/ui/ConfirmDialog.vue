<script setup lang="ts">
import { AlertTriangle, X } from 'lucide-vue-next';

defineProps<{
  show: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  type?: 'danger' | 'warning' | 'info';
}>();

const emit = defineEmits(['confirm', 'cancel']);
</script>

<template>
  <Transition name="fade">
    <div v-if="show" class="dialog-overlay flex-center" @click="emit('cancel')">
      <div class="dialog-card glass anim-scale-in" @click.stop>
        <div class="dialog-header flex-between" :class="type || 'warning'">
          <div class="flex-center gap-2">
            <AlertTriangle :size="20" />
            <span class="font-bold">{{ title }}</span>
          </div>
          <button class="icon-btn" @click="emit('cancel')">
            <X :size="18" />
          </button>
        </div>
        
        <div class="dialog-body">
          <p v-html="message" />
        </div>
        
        <div class="dialog-footer flex-end gap-2">
          <button class="button-secondary" @click="emit('cancel')">
            {{ cancelLabel || 'Cancel' }}
          </button>
          <button 
            :class="type === 'danger' ? 'button-danger' : 'button-primary'" 
            @click="emit('confirm')"
          >
            {{ confirmLabel || 'Confirm' }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: var(--overlay-bg);
  backdrop-filter: blur(4px);
  z-index: 2000;
  padding: 20px;
}

.dialog-card {
  width: 100%;
  max-width: 400px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--shadow-xl);
}

.dialog-header {
  padding: 16px;
  border-bottom: 1px solid var(--border-color);
}

.dialog-header.warning { color: var(--text-warning); background: var(--bg-warning); }
.dialog-header.danger { color: var(--text-error); background: var(--bg-error); }
.dialog-header.info { color: var(--accent-primary); background: rgba(59, 130, 246, 0.05); }

.dialog-body {
  padding: 20px 16px;
  color: var(--text-primary);
  line-height: 1.5;
  font-size: 0.95rem;
}

.dialog-body :deep(.confirm-name) {
  font-family: var(--font-mono);
  font-weight: 700;
  color: #f87171;
  background: rgba(248, 113, 113, 0.1);
  padding: 1px 5px;
  border-radius: 4px;
}

.dialog-body :deep(.confirm-name-list) {
  display: block;
  margin-top: 6px;
  font-family: var(--font-mono);
  font-size: 0.8rem;
  color: var(--text-secondary);
  opacity: 0.8;
}

.dialog-footer {
  padding: 12px 16px;
  background: var(--bg-tertiary);
  border-top: 1px solid var(--border-color);
}

.button-danger {
  background: var(--text-error);
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 6px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.button-danger:hover {
  opacity: 0.9;
  transform: translateY(-1px);
}

.flex-end {
  display: flex;
  justify-content: flex-end;
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

.anim-scale-in {
  animation: scaleIn 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes scaleIn {
  from { opacity: 0; transform: scale(0.9) translateY(20px); }
  to { opacity: 1; transform: scale(1) translateY(0); }
}
</style>
