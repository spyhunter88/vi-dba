<script setup lang="ts">
import { computed, ref } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { X, Plus, Terminal, Table as TableIcon, Layout, ChevronRight, ChevronDown, List } from 'lucide-vue-next';
import type { Tab } from '../../types';

const tabStore = useTabStore();
const connectionStore = useConnectionStore();

// Helper for connection colors
function getConnectionColor(connectionId: string | undefined) {
  if (!connectionId) return 'transparent';
  let hash = 0;
  for (let i = 0; i < connectionId.length; i++) {
    hash = connectionId.charCodeAt(i) + ((hash << 5) - hash);
  }
  const h = Math.abs(hash) % 360;
  return `hsla(${h}, 70%, 50%, 0.15)`;
}

// Grouped tabs for display
const groups = computed(() => {
  const result: { connectionId: string; name: string; color: string; tabs: Tab[]; isCollapsed: boolean }[] = [];
  
  tabStore.tabs.forEach(tab => {
    let group = result.find(g => g.connectionId === tab.connectionId);
    if (!group) {
      const conn = connectionStore.connections.find(c => c.id === tab.connectionId);
      group = {
        connectionId: tab.connectionId,
        name: conn ? conn.name : (tab.connectionId ? 'Unknown' : 'No Connection'),
        color: getConnectionColor(tab.connectionId),
        tabs: [],
        isCollapsed: tabStore.collapsedTabGroups.has(tab.connectionId)
      };
      result.push(group);
    }
    group.tabs.push(tab);
  });
  
  return result;
});

// Drag and drop state
const draggedTabId = ref<string | null>(null);

function handleDragStart(e: DragEvent, id: string) {
  draggedTabId.value = id;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', id);
    // Add a ghost image or just styling
    const target = e.target as HTMLElement;
    target.style.opacity = '0.4';
  }
}

function handleDragEnd(e: DragEvent) {
  const target = e.target as HTMLElement;
  target.style.opacity = '1';
  draggedTabId.value = null;
}

function handleDragOver(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move';
  }
}

function handleDrop(e: DragEvent, targetId: string) {
  e.preventDefault();
  if (draggedTabId.value && draggedTabId.value !== targetId) {
    tabStore.reorderTabs(draggedTabId.value, targetId);
  }
}

function handleCloseTab(e: Event, id: string) {
  e.stopPropagation();
  tabStore.closeTab(id);
}

function handleMiddleClick(e: MouseEvent, id: string) {
  if (e.button === 1) { // Middle button
    e.preventDefault();
    tabStore.closeTab(id);
  }
}

function handleNewQueryTab() {
  // Inherit the connection (and context) of the tab the user is currently on,
  // so a new query opens in the active tab-group rather than whichever connection
  // is selected in the sidebar.
  const active = tabStore.activeTab;
  const newTab: Tab = {
    id: Math.random().toString(36).substring(7),
    title: `Query ${tabStore.tabs.filter(t => t.type === 'sql_query').length + 1}`,
    type: 'sql_query',
    connectionId: active?.connectionId || connectionStore.activeConnectionId || '',
    database: active?.database,
    schema: active?.schema,
    content: '',
  };
  tabStore.addTab(newTab);
}

function isGroupActive(groupTabs: Tab[]) {
  return groupTabs.some(t => t.id === tabStore.activeTabId);
}
</script>

<template>
  <div class="tab-bar-container">
    <div class="tab-bar glass">
      <div 
        v-for="group in groups" 
        :key="group.connectionId" 
        class="tab-group"
        :style="{ '--group-color': group.color }"
      >
        <button 
          class="collapse-toggle" 
          @click="tabStore.toggleTabGroupCollapse(group.connectionId)"
          :title="group.isCollapsed ? 'Expand Group' : 'Collapse Group'"
        >
          <ChevronDown v-if="!group.isCollapsed" :size="14" />
          <ChevronRight v-else :size="14" />
        </button>

        <!-- Collapsed Summary Tab -->
        <div 
          v-if="group.isCollapsed"
          class="tab-item collapsed-summary"
          :class="{ active: isGroupActive(group.tabs) }"
          :style="{ backgroundColor: group.color }"
          @click="tabStore.toggleTabGroupCollapse(group.connectionId)"
        >
          <List :size="14" class="tab-icon" />
          <span class="tab-title">{{ group.name }}</span>
          <span class="tab-count">{{ group.tabs.length }}</span>
        </div>

        <!-- Expanded Tabs -->
        <template v-else>
          <div 
            v-for="tab in group.tabs" 
            :key="tab.id" 
            class="tab-item"
            :class="{ active: tabStore.activeTabId === tab.id }"
            :style="{ backgroundColor: group.color }"
            draggable="true"
            @click="tabStore.activateTab(tab.id)"
            @mousedown="handleMiddleClick($event, tab.id)"
            @dragstart="handleDragStart($event, tab.id)"
            @dragend="handleDragEnd"
            @dragover="handleDragOver"
            @drop="handleDrop($event, tab.id)"
          >
            <Terminal v-if="tab.type === 'sql_query'" :size="14" class="tab-icon" />
            <TableIcon v-else-if="tab.type === 'table_data'" :size="14" class="tab-icon" />
            <Layout v-else :size="14" class="tab-icon" />
            
            <span class="tab-title">{{ tab.title }}</span>
            <span v-if="tab.isDirty" class="dirty-indicator">*</span>
            
            <button class="close-btn" @click="(e) => handleCloseTab(e, tab.id)">
              <X :size="12" />
            </button>
          </div>
        </template>
      </div>

      <button class="new-tab-btn" @click="handleNewQueryTab" title="New Query Tab">
        <Plus :size="16" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.tab-bar-container {
  height: 40px;
  width: 100%;
  display: flex;
}

.tab-bar {
  flex: 1;
  display: flex;
  align-items: center;
  padding: 0 4px; /* Reduced from 8px to account for toggles */
  gap: 2px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
  overflow-x: auto;
  scrollbar-width: none; /* Firefox */
}

.tab-bar::-webkit-scrollbar {
  display: none; /* Chrome/Safari */
}

.tab-group {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 100%;
  padding: 0 4px;
  position: relative;
}

.tab-group:not(:last-of-type)::after {
  content: '';
  position: absolute;
  right: -2px;
  top: 20%;
  bottom: 20%;
  width: 1px;
  background: var(--border-color);
  opacity: 0.5;
}

.tab-item {
  display: flex;
  align-items: center;
  height: 32px;
  padding: 0 12px;
  gap: 8px;
  border-radius: 6px 6px 0 0;
  cursor: pointer;
  font-size: 0.85rem;
  color: var(--text-secondary);
  max-width: 200px;
  transition: all 0.2s;
  position: relative;
  user-select: none;
}

.tab-item:hover {
  filter: brightness(1.2);
  color: var(--text-primary);
}

.tab-item.active {
  color: var(--text-primary);
}

.tab-item.active::after {
  content: '';
  position: absolute;
  bottom: -1px;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--accent-primary);
}

.collapsed-summary {
  min-width: 120px;
  border: 1px dashed var(--border-color);
}

.tab-count {
  background: var(--accent-primary);
  color: white;
  font-size: 0.7rem;
  padding: 0 6px;
  border-radius: 10px;
  font-weight: bold;
}

.collapse-toggle {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  display: flex;
  border-radius: 4px;
  opacity: 0.6;
  transition: all 0.2s;
}

.collapse-toggle:hover {
  opacity: 1;
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-primary);
}

.tab-icon {
  opacity: 0.7;
  flex-shrink: 0;
}

.tab-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dirty-indicator {
  color: var(--accent-primary);
  font-weight: bold;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  opacity: 0;
  transition: opacity 0.2s, background 0.2s;
}

.tab-item:hover .close-btn {
  opacity: 1;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #ef4444;
}

.new-tab-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 6px;
  border-radius: 4px;
  display: flex;
  margin-left: 4px;
  transition: all 0.2s;
  flex-shrink: 0;
}

.new-tab-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}
</style>
