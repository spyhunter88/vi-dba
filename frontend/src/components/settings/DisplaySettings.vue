<script setup lang="ts">
import { Sun, Moon, Monitor } from 'lucide-vue-next';
import { useUiStore } from '../../stores/ui';

const uiStore = useUiStore();
</script>

<template>
  <div class="settings-content">
    <h2>Display</h2>
    <div class="settings-section">
      <div class="settings-row">
        <div class="row-label">
          <span>Appearance</span>
          <p>Select how ViDBConnect looks on your screen.</p>
        </div>
        <div class="row-action">
          <div class="theme-picker">
            <button 
              class="theme-btn" 
              :class="{ active: uiStore.theme === 'light' }"
              @click="uiStore.setTheme('light')"
            >
              <Sun :size="18" />
              <span>Light</span>
            </button>
            <button 
              class="theme-btn" 
              :class="{ active: uiStore.theme === 'dark' }"
              @click="uiStore.setTheme('dark')"
            >
              <Moon :size="18" />
              <span>Dark</span>
            </button>
            <button 
              class="theme-btn" 
              :class="{ active: uiStore.theme === 'system' }"
              @click="uiStore.setTheme('system')"
            >
              <Monitor :size="18" />
              <span>System</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-content {
  padding: 24px;
}

h2 {
  font-size: 1.25rem;
  margin-bottom: 24px;
  color: var(--text-primary);
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.settings-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.row-label span {
  font-weight: 600;
  display: block;
  margin-bottom: 4px;
}

.row-label p {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.theme-picker {
  display: flex;
  gap: 8px;
  background: var(--bg-tertiary);
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
}

.theme-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s;
  min-width: 80px;
}

.theme-btn span {
  font-size: 0.75rem;
  font-weight: 500;
}

.theme-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.theme-btn.active {
  background: var(--accent-primary);
  color: white;
}
</style>
