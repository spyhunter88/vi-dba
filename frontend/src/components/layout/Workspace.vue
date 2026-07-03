<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useConnectionStore } from '../../stores/connections';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { ChevronRight, Plus, Unplug, Link, Database, Sparkles, X } from 'lucide-vue-next';
import SqlEditor from '../editor/SqlEditor.vue';
import ResultPane from '../editor/ResultPane.vue';
import TableView from '../editor/TableView.vue';
import Welcome from '../../views/Welcome.vue';
import TableList from '../editor/TableList.vue';
import TableEditor from '../editor/TableEditor.vue';
import ScriptList from '../editor/ScriptList.vue';
import RoutineList from '../editor/RoutineList.vue';
import RoutineEditor from '../editor/RoutineEditor.vue';
import ViewEditor from '../editor/ViewEditor.vue';
import AiSqlEditor from '../editor/AiSqlEditor.vue';
import AiRoutineEditor from '../editor/AiRoutineEditor.vue';
import type { DbType } from '../../types';

const connectionStore = useConnectionStore();
const tabStore = useTabStore();
const queryStore = useQueryStore();
const activeTab = computed(() => tabStore.activeTab);
const emit = defineEmits<{
  (e: 'add-connection', type: DbType): void
}>();

const splitPercent = ref(60); // Percentage of height for editor
const isResizing = ref(false);
const dismissedOverlayTabIds = ref<Set<string>>(new Set());

function startResizing() {
  isResizing.value = true;
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResizing);
}

function stopResizing() {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResizing);
}

function handleMouseMove(e: MouseEvent) {
  if (!isResizing.value) return;
  const workspace = document.querySelector('.workspace');
  if (!workspace) return;
  
  const rect = workspace.getBoundingClientRect();
  const y = e.clientY - rect.top;
  const h = rect.height;
  
  splitPercent.value = Math.max(10, Math.min(90, (y / h) * 100));
}

// Clear dismissed overlay when a connection becomes active again (so re-disconnect shows it again)
watch(() => connectionStore.connectedIds, (newIds) => {
  dismissedOverlayTabIds.value.forEach(tabId => {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (tab && tab.connectionId && newIds.has(tab.connectionId)) {
      dismissedOverlayTabIds.value.delete(tabId);
    }
  });
}, { deep: true });

function handleReconnect(connectionId: string) {
  if (tabStore.tabToReconnect) {
    tabStore.tabToReconnect.connectionId = connectionId;
    tabStore.tabToReconnect.isDetached = false;
    tabStore.closeReconnectModal();
    
    // Refresh if it's a table view
    if (tabStore.tabToReconnect.type === 'table_data') {
      queryStore.executeQueryInTab(tabStore.tabToReconnect.id);
    }
  }
}
</script>

<template>
  <main class="workspace">
    <div v-if="!tabStore.activeTabId" class="h-full w-full">
      <Welcome @add-connection="(type) => $emit('add-connection', type)" />
    </div>

    <template v-else-if="activeTab">
      <!-- All tabs use v-show so components stay mounted — switching tabs is a CSS-only toggle,
           not a destroy/create cycle. This is critical for Monaco editor tabs whose init cost
           is high enough to cause visible lag on every switch. -->
      <div
        v-for="tab in tabStore.tabs"
        :key="tab.id"
        v-show="tabStore.activeTabId === tab.id"
        class="tab-content h-full"
        @mousedown="tabStore.activateTab(tab.id)"
      >
        <!-- SQL Query tabs -->
        <div v-if="['sql_query', 'query'].includes(tab.type)" class="main-split h-full" :class="{ 'ai-open': tab.aiPromptOpen }">
          <div class="upper-pane h-full overflow-hidden" :style="{ height: splitPercent + '%' }">
            <SqlEditor v-model="tab.content" :tab-id="tab.id" />
          </div>
          <div class="splitter" @mousedown="startResizing">
            <div class="splitter-handle"></div>
          </div>
          <div class="lower-pane min-h-0 overflow-hidden" :style="{ height: (100 - splitPercent) + '%' }">
            <ResultPane :tab-id="tab.id" />
          </div>
          <!-- Reconnect Overlay -->
          <div v-if="tab.connectionId && !connectionStore.connectedIds.has(tab.connectionId) && !dismissedOverlayTabIds.has(tab.id)" class="reconnect-overlay absolute inset-0 z-2000 flex-center">
            <div class="reconnect-card flex-center flex-col gap-3">
              <button class="reconnect-close" @click="dismissedOverlayTabIds.add(tab.id)" :title="'Dismiss'">
                <X :size="16" />
              </button>
              <Unplug :size="40" class="reconnect-icon" />
              <h3 class="reconnect-title">Disconnected</h3>
              <p class="reconnect-desc">Connection required to view or run queries.</p>
              <div class="flex gap-2 mt-2">
                <button class="button-primary" @click="() => connectionStore.connect(tab.connectionId!)">
                  Reconnect
                </button>
                <button class="button-outline" @click="() => tabStore.openReconnectModal(tab)">
                  Switch Connection
                </button>
              </div>
            </div>
          </div>
        </div>

        <template v-else-if="tab.type === 'table_list'">
          <div class="table-list-wrapper h-full">
            <TableList :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'table_data'">
          <div class="table-data-wrapper h-full">
            <TableView :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'table_structure'">
          <div class="table-editor-wrapper h-full">
            <TableEditor :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'script_list'">
          <div class="script-list-wrapper h-full">
            <ScriptList :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'routine_editor'">
          <div class="routine-editor-wrapper h-full">
            <RoutineEditor :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'view_editor'">
          <div class="view-editor-wrapper h-full">
            <ViewEditor :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'ai_sql'">
          <div class="ai-sql-editor-wrapper h-full">
            <AiSqlEditor :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'ai_routine_editor'">
          <div class="ai-routine-editor-wrapper h-full">
            <AiRoutineEditor :tab-id="tab.id" />
          </div>
        </template>
        <template v-else-if="tab.type === 'routine_list'">
          <div class="routine-list-wrapper h-full">
            <RoutineList :tab-id="tab.id" />
          </div>
        </template>
        <template v-else>
          <!-- Default fallback for any multi-pane tab that has results -->
          <div class="flex flex-col h-full relative">
            <div v-if="tab.isDetached" class="detached-banner flex-between">
              <div class="flex-center gap-2">
                <Unplug :size="14" />
                <span>This view is detached.</span>
              </div>
              <button class="button-warning sm" @click="tabStore.openReconnectModal(tab)">Reconnect</button>
            </div>
            <div class="editor-toolbar flex-between glass" v-else>
              <div class="flex-center gap-4">
                <div class="connection-info flex-center gap-2">
                  <Database :size="14" class="text-accent" />
                  <div class="flex flex-col">
                    <span class="text-xs font-bold text-accent leading-tight">{{ connectionStore.connections.find(c => c.id === tab.connectionId)?.name || 'Unknown' }}</span>
                    <span v-if="tab.database || tab.schema" class="text-9px opacity-70 text-accent leading-none">
                      {{ tab.database || tab.schema }}{{ tab.database && tab.schema ? '.' + tab.schema : '' }}
                    </span>
                  </div>
                  <template v-if="!connectionStore.connectedIds.has(tab.connectionId)">
                    <span class="text-xs text-muted-foreground italic">(Disconnected)</span>
                    <button class="button-primary sm h-6 px-2 text-xs" @click="connectionStore.connect(tab.connectionId)">Connect</button>
                  </template>
                </div>
                <button class="button-ai-sparkle sm" @click="tabStore.openAiSqlEditor(tab.connectionId!, tab.database, tab.schema)">
                  <Sparkles :size="14" />
                  Ask AI
                </button>
              </div>
            </div>
            <div class="flex-1 overflow-hidden flex flex-col min-h-0">
              <ResultPane :tab-id="tab.id" />
            </div>
          </div>
        </template>
      </div>
    </template>
    
    <!-- Reconnect Modal (Global for Workspace) -->
    <div v-if="tabStore.showReconnectModal" class="absolute inset-0 z-50 flex-center glass-overlay">
       <div class="glass-panel p-6 flex flex-col gap-4 min-w-[340px] anim-scale-in">
          <div class="flex items-center gap-3">
            <div class="icon-circle bg-accent text-white"><Link :size="18" /></div>
            <h3 class="font-bold text-lg">Attach Connection</h3>
          </div>
          <p class="text-sm text-secondary">Select a connection to associate with this tab:</p>
          <div class="flex flex-col gap-2 max-h-[300px] overflow-y-auto pr-1">
             <button 
                v-for="conn in connectionStore.connections" 
                :key="conn.id"
                class="connection-choice-item flex items-center justify-between p-3 rounded-lg border border-transparent hover:border-accent hover:bg-accent-transparent transition-all"
                @click="handleReconnect(conn.id)"
             >  
                <div class="flex items-center gap-3">
                  <Database :size="16" class="opacity-60" />
                  <span class="font-medium">{{ conn.name }}</span>
                </div>
                <ChevronRight :size="14" class="opacity-40" />
             </button>
             <div v-if="connectionStore.connections.length === 0" class="text-xs text-center p-8 opacity-50 border border-dashed rounded-lg">
                No connections available.
             </div>
          </div>
          <div class="flex justify-between items-center mt-2">
            <button class="button-ghost sm" @click="connectionStore.openNewConnectionModal()">
              <Plus :size="14" /> New Connection
            </button>
            <button class="button-outline sm" @click="tabStore.closeReconnectModal()">Cancel</button>
          </div>
       </div>
    </div>
  </main>
</template>

<style scoped>
.workspace {
  flex: 1;
  overflow: hidden;
  position: relative;
  background: var(--bg-tertiary);
}

.tab-content {
  height: 100%;
  width: 100%;
}

.main-split {
  display: flex;
  flex-direction: column;
}

.upper-pane {
  min-height: 100px;
}

.lower-pane {
  min-height: 100px;
  display: flex;
  flex-direction: column;
}

.splitter {
  height: 6px;
  background: var(--border-color);
  cursor: row-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 5;
  transition: background 0.2s;
  flex-shrink: 0;
}

.splitter:hover {
  background: var(--accent-primary);
}

.splitter-handle {
  width: 40px;
  height: 3px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
}

.editor-toolbar {
  height: 40px;
  padding: 0 12px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
  flex-shrink: 0;
}

.detached-banner {
  background: rgba(234, 179, 8, 0.1);
  color: rgb(234, 179, 8);
  padding: 8px 12px;
  border-bottom: 1px solid rgba(234, 179, 8, 0.2);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
}

.glass-overlay {
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
}

.glass-panel {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: var(--shadow-2xl);
}

.connection-choice-item:hover {
  background: rgba(var(--accent-rgb, 59, 130, 246), 0.1);
  border-color: var(--accent-primary);
}

.icon-circle {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--accent-primary);
}


.reconnect-overlay {
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(3px);
}

.reconnect-card {
  position: relative;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  padding: 36px 40px 28px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  text-align: center;
  min-width: 280px;
}

.reconnect-close {
  position: absolute;
  top: 10px;
  right: 10px;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s;
}

.reconnect-close:hover {
  background: var(--bg-tertiary);
  color: var(--text-primary);
}

.reconnect-icon {
  color: var(--text-secondary);
  opacity: 0.45;
}

.reconnect-title {
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
}

.reconnect-desc {
  font-size: 0.8rem;
  color: var(--text-secondary);
  margin: 0;
}
</style>
