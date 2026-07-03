<script setup lang="ts">
import { Plus, Edit, FileCode, FolderOpen } from 'lucide-vue-next';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { computed } from 'vue';

const tabStore = useTabStore();
const connectionStore = useConnectionStore();

const canEditTable = computed(() => {
  const tab = tabStore.activeTab;
  return tab?.type === 'table_data' || tab?.type === 'table_list';
});

function handleCreateTable() {
  if (connectionStore.activeConnectionId && connectionStore.activeDatabase) {
    tabStore.createTable(connectionStore.activeConnectionId, connectionStore.activeDatabase, connectionStore.activeSchema || undefined);
  }
}

function handleCreateScript() {
  if (connectionStore.activeConnectionId && connectionStore.activeDatabase) {
    tabStore.createScript(connectionStore.activeConnectionId, connectionStore.activeDatabase, connectionStore.activeSchema || undefined);
  }
}

function handleOpenScripts() {
  if (connectionStore.activeConnectionId) {
    tabStore.openScriptList(connectionStore.activeConnectionId);
  }
}

function handleEditTable() {
  const tab = tabStore.activeTab;
  if (tab?.connectionId && tab.metadata?.tableName) {
    tabStore.editTable(tab.connectionId, tab.metadata.tableName, tab.metadata.catalog, tab.metadata.schema);
  } else {
    console.warn('No active table to edit');
  }
}
</script>

<template>
  <div class="toolbar glass">
    <div class="toolbar-section">
      <button 
        class="toolbar-btn" 
        :disabled="!connectionStore.activeConnectionId || !connectionStore.activeDatabase"
        @click="handleCreateTable"
        title="Create Table"
      >
        <Plus :size="18" />
        <span>Create Table</span>
      </button>

      <button 
        class="toolbar-btn" 
        :disabled="!connectionStore.activeConnectionId || !connectionStore.activeDatabase"
        @click="handleCreateScript"
        title="New Script"
      >
        <FileCode :size="18" />
        <span>New Script</span>
      </button>

      <button 
        class="toolbar-btn" 
        :disabled="!connectionStore.activeConnectionId || !connectionStore.activeDatabase"
        @click="handleOpenScripts"
        title="Saved Scripts"
      >
        <FolderOpen :size="18" />
        <span>Scripts</span>
      </button>
      
      <button 
        class="toolbar-btn" 
        :disabled="!canEditTable"
        @click="handleEditTable"
        title="Edit Table"
      >
        <Edit :size="18" />
        <span>Edit Table</span>
      </button>

      <div class="divider"></div>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  height: 48px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  gap: 8px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
  z-index: 20;
}

.toolbar-section {
  display: flex;
  align-items: center;
  gap: 4px;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-primary);
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.85rem;
  font-weight: 500;
  transition: all 0.2s;
}

.toolbar-btn:hover:not(:disabled) {
  background: var(--glass-border);
  border-color: var(--border-color);
}

.toolbar-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.divider {
  width: 1px;
  height: 24px;
  background: var(--border-color);
  margin: 0 8px;
}

.toolbar-btn span {
  white-space: nowrap;
}
</style>
