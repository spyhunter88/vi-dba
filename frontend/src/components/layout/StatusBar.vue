<script setup lang="ts">
import { computed } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { Wifi, WifiOff, Clock, Activity, Database } from 'lucide-vue-next';

const tabStore = useTabStore();
const connectionStore = useConnectionStore();

// Active database display follows the same priority as query execution:
// 1. Database set on the current tab (opened from sidebar or script)
// 2. Default database of the active connection
// 3. Nothing shown
const currentTabConnection = computed(() => {
  const tab = tabStore.activeTab;
  if (tab && tab.connectionId) {
    return connectionStore.connections.find(c => c.id === tab.connectionId);
  }
  return connectionStore.activeConnection;
});

const isConnected = computed(() => {
  if (currentTabConnection.value) {
    return connectionStore.connectedIds.has(currentTabConnection.value.id);
  }
  return false;
});

// Active database display follows the same priority as query execution:
// 1. Database set on the current tab (opened from sidebar or script)
// 2. Default database of the active connection
// 3. Nothing shown
const activeDisplayDatabase = computed(() => {
  const tab = tabStore.activeTab;
  return tab?.database || currentTabConnection.value?.database || null;
});

const activeDisplaySchema = computed(() => tabStore.activeTab?.schema || null);
</script>

<template>
  <footer class="status-bar glass">
    <div class="status-left">
      <div class="status-item">
        <Wifi v-if="isConnected" :size="12" class="text-success" />
        <WifiOff v-else :size="12" class="text-secondary" />
        <span class="font-bold text-accent">{{ currentTabConnection?.name || 'Disconnected' }}</span>
      </div>
      <div v-if="currentTabConnection" class="status-item">
        <Activity :size="12" />
        <span>{{ currentTabConnection.dbType }}</span>
      </div>
      <div v-if="activeDisplayDatabase || activeDisplaySchema" class="status-item">
        <Database :size="12" />
        <span>{{ [activeDisplayDatabase, activeDisplaySchema].filter(Boolean).join('.') }}</span>
      </div>
    </div>
    
    <div class="status-right">
      <div v-if="tabStore.activeTab" class="status-item">
        <Clock :size="12" />
        <span>Auto-commit</span>
      </div>
      <div class="status-item">
        <span>UTF-8</span>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  height: 24px;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  font-size: 0.7rem;
  color: var(--text-secondary);
  border-top: 1px solid var(--border-color);
  background: var(--bg-primary);
  z-index: 10;
}

.status-left, .status-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.text-success {
  color: var(--text-success);
}

.text-secondary {
  color: var(--text-secondary);
}
</style>
