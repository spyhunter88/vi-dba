<script setup lang="ts">
import { ref, onBeforeUnmount, watch } from 'vue';

interface ContextMenuItem {
  label: string;
  icon?: any;
  action: () => void;
  disabled?: boolean;
  shortcut?: string;
  danger?: boolean;
}

const props = defineProps<{
  x: number;
  y: number;
  show: boolean;
  items: ContextMenuItem[];
}>();

const emit = defineEmits(['close']);

const menuRef = ref<HTMLElement | null>(null);

function handleClose() {
  emit('close');
}

// Close on click outside.
// Listeners are attached only while the menu is visible, and the attach is deferred to the
// next macrotask so the same contextmenu/click event that opened the menu cannot also close it.
function handleOutsideClick(e: MouseEvent) {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    handleClose();
  }
}

let attachTimer: ReturnType<typeof setTimeout> | null = null;

function attachListeners() {
  document.addEventListener('mousedown', handleOutsideClick);
  document.addEventListener('contextmenu', handleOutsideClick);
}

function detachListeners() {
  if (attachTimer !== null) {
    clearTimeout(attachTimer);
    attachTimer = null;
  }
  document.removeEventListener('mousedown', handleOutsideClick);
  document.removeEventListener('contextmenu', handleOutsideClick);
}

watch(() => props.show, (visible) => {
  detachListeners();
  if (visible) {
    attachTimer = setTimeout(attachListeners, 0);
  }
});

onBeforeUnmount(detachListeners);

// Adjust position to keep it inside viewport
const adjustedX = ref(0);
const adjustedY = ref(0);

watch(() => [props.x, props.y, props.show], () => {
  if (props.show) {
    // Initial position
    adjustedX.value = props.x;
    adjustedY.value = props.y;
    
    // Wait for next tick to get menu dimensions
    setTimeout(() => {
      if (!menuRef.value) return;
      const rect = menuRef.value.getBoundingClientRect();
      const winWidth = window.innerWidth;
      const winHeight = window.innerHeight;
      
      if (props.x + rect.width > winWidth) {
        adjustedX.value = winWidth - rect.width - 10;
      }
      
      if (props.y + rect.height > winHeight) {
        adjustedY.value = winHeight - rect.height - 10;
      }
    }, 0);
  }
}, { immediate: true });

function onItemClick(item: ContextMenuItem) {
  if (item.disabled) return;
  item.action();
  handleClose();
}
</script>

<template>
  <Teleport to="body">
    <transition name="menu-fade">
      <div 
        v-if="show"
        ref="menuRef"
        class="context-menu glass shadow-2xl"
        :style="{ top: adjustedY + 'px', left: adjustedX + 'px' }"
      >
        <div class="menu-items">
          <template v-for="(item, idx) in items" :key="idx">
            <div 
              class="menu-item"
              :class="{ 'disabled': item.disabled, 'danger': item.danger }"
              @click.stop="onItemClick(item)"
            >
              <div class="item-left">
                <component v-if="item.icon" :is="item.icon" :size="16" class="item-icon" />
                <span class="item-label">{{ item.label }}</span>
              </div>
              <span v-if="item.shortcut" class="item-shortcut">{{ item.shortcut }}</span>
            </div>
          </template>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 180px;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  backdrop-filter: blur(12px) saturate(180%);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 6px;
  box-shadow: var(--shadow-lg);
  animation: menu-pop 0.15s cubic-bezier(0.2, 0, 0.2, 1);
}

.menu-items {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.1s ease;
  user-select: none;
}

.menu-item:hover:not(.disabled) {
  background: rgba(var(--accent-primary-rgb, 59, 130, 246), 0.15);
  color: var(--accent-primary, #3b82f6);
}

.menu-item.disabled {
  opacity: 0.4;
  cursor: default;
}

.menu-item.danger:hover {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.item-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.item-icon {
  opacity: 0.7;
}

.item-label {
  font-size: 0.85rem;
  font-weight: 500;
}

.item-shortcut {
  font-size: 0.7rem;
  opacity: 0.4;
  font-family: var(--font-mono, monospace);
  margin-left: 12px;
}

/* Animations */
@keyframes menu-pop {
  from {
    opacity: 0;
    transform: scale(0.95) translateY(-5px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.menu-fade-enter-active,
.menu-fade-leave-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}

.menu-fade-enter-from,
.menu-fade-leave-to {
  opacity: 0;
  transform: scale(0.98);
}
</style>
