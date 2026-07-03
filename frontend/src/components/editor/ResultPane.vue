<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { 
  List, MessageSquare, Clipboard, Scaling, 
  AlertCircle, Plus, Edit, RefreshCw, 
  Terminal, Download, Database
} from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';
import { useConnectionStore } from '../../stores/connections';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import type { QueryResult } from '../../types';
import DataGrid from './DataGrid.vue';

const props = defineProps<{
  tabId: string;
  result?: {
    loading: boolean;
    results: QueryResult[];
    error: string | null;
    messages?: string[];
    totalTimeMs?: number;
    execId?: string;
    startTime?: number;
    elapsedMs?: number;
  };
}>();

const connectionStore = useConnectionStore();
const tabStore = useTabStore();
const queryStore = useQueryStore();

const activePaneTab = ref<'results' | 'messages' | 'plan'>('results');
const selectedResultIndex = ref(0);
const dataGridRef = ref<InstanceType<typeof DataGrid> | null>(null);

const displayResult = computed(() => {
  if (props.result) return props.result;
  return queryStore.queryResults[props.tabId];
});

const currentResult = computed(() => {
  if (!displayResult.value?.results || displayResult.value.results.length === 0) return null;
  return displayResult.value.results[selectedResultIndex.value];
});

// Reset selected index when results change
watch(() => displayResult.value?.results, (newResults) => {
  if (newResults && newResults.length > 0) {
    selectedResultIndex.value = 0;
  }
});

const isEditable = computed(() => {
  const data = currentResult.value;
  return data && data.primaryKeys && data.primaryKeys.length > 0 && !!data.tableName;
});

function addNewRow() {
  dataGridRef.value?.addNewRow();
}

const tab = computed(() => tabStore.tabs.find(t => t.id === props.tabId));

function handleCreateTable() {
  const connId = tab.value?.connectionId;
  if (connId) {
    tabStore.createTable(connId, tab.value?.metadata?.catalog, tab.value?.metadata?.schema);
  }
}

function handleEditTable() {
  const data = currentResult.value;
  const connId = tab.value?.connectionId;
  if (connId && data?.tableName) {
    tabStore.editTable(connId, data.tableName, tab.value?.metadata?.catalog, tab.value?.metadata?.schema);
  }
}

async function handleExport() {
  const connId = tab.value?.connectionId;
  const data = currentResult.value;
  if (connId && data) {
    try {
      await invoke('set_export_data', { data });
      invoke('open_export', {
        connectionId: connId,
        sourceType: 'current',
        sourceName: data?.tableName || null,
        query: tab.value?.metadata?.query || null,
        isCurrent: true
      });
    } catch (e) {
      console.error('Failed to prepare export data:', e);
    }
  }
}

function handleEditConnection() {
  const conn = connectionStore.connections.find(c => c.id === tab.value?.connectionId);
  if (conn) {
    connectionStore.openEditConnectionModal(conn);
  }
}

function formatElapsed(ms?: number): string {
  if (ms === undefined || ms === null) return '0 ms';
  if (ms < 1000) return `${ms} ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(2)} s`;
  const m = Math.floor(sec / 60);
  const s = (sec - m * 60).toFixed(1);
  return `${m}m ${s}s`;
}

function checkAndLoad() {
  const currentTab = tab.value;
  console.log(`[ResultPane] checkAndLoad for ${props.tabId}. Tab type: ${currentTab?.type}`);
  
  if (currentTab?.type === 'table_data') {
    const existingResult = queryStore.queryResults[props.tabId];
    console.log(`[ResultPane] existingResult loading status: ${existingResult?.loading}`);
    
    if (!existingResult || (existingResult.results.length === 0 && !existingResult.loading && !existingResult.error)) {
      console.log(`[ResultPane] Calling executeQueryInTab for ${props.tabId}`);
      queryStore.executeQueryInTab(props.tabId);
    }
  }
}

watch(() => props.tabId, (newId) => {
  console.log(`[ResultPane] TabId changed to ${newId}. State:`, queryStore.queryResults[newId]);
  checkAndLoad();
});

onMounted(() => {
  console.log(`[ResultPane] Mounted for ${props.tabId}. State:`, queryStore.queryResults[props.tabId]);
  checkAndLoad();
});
</script>

<template>
  <div class="result-pane glass">
    <div class="result-toolbar flex-between">
      <div class="pane-tabs flex-center">
        <button 
          class="pane-tab" 
          :class="{ active: activePaneTab === 'results' }"
          @click="activePaneTab = 'results'"
        >
          <List :size="14" />
          Results
        </button>
        <button 
          class="pane-tab" 
          :class="{ active: activePaneTab === 'messages' }"
          @click="activePaneTab = 'messages'"
        >
          <MessageSquare :size="14" />
          Messages <span v-if="displayResult?.results?.length" class="badge">{{ displayResult.results.length }}</span>
        </button>

        <!-- Inline Results Sub-tabs -->
        <div v-if="activePaneTab === 'results' && displayResult?.results && displayResult.results.length > 1" class="separator"></div>
        <div v-if="activePaneTab === 'results' && displayResult?.results && displayResult.results.length > 1" class="inline-sub-tabs">
           <button 
            v-for="(_, idx) in displayResult.results" 
            :key="idx"
            class="result-chip"
            :class="['color-' + (idx % 4), { active: selectedResultIndex === idx }]"
            @click="selectedResultIndex = idx"
            :title="'Result ' + (idx + 1)"
          >
            {{ idx + 1 }}
          </button>
        </div>
      </div>
      
      <div class="pane-actions flex-center gap-2">
        <button 
          v-if="isEditable"
          class="icon-btn" 
          title="Add new row"
          @click="addNewRow"
        >
          <Plus :size="14" />
        </button>
        <button 
          class="icon-btn" 
          title="Refresh data"
          @click="queryStore.executeQueryInTab(tabId)"
        >
          <RefreshCw :size="14" />
        </button>
        <button 
          v-if="tab?.type === 'table_data' && currentResult?.tableName"
          class="icon-btn" 
          title="Edit table structure"
          @click="handleEditTable"
        >
          <Edit :size="14" />
        </button>
        <button 
          v-if="tab?.type === 'table_data' && tab?.connectionId"
          class="icon-btn" 
          title="Create new table"
          @click="handleCreateTable"
        >
          <Plus :size="14" class="text-accent" />
        </button>
        <button 
          class="icon-btn" 
          title="Export results"
          @click="handleExport"
        >
          <Download :size="14" />
        </button>
        <button class="icon-btn" title="Copy results">
          <Clipboard :size="14" />
        </button>
      </div>
    </div>



    <div class="pane-content">
      <div v-if="displayResult?.loading || (tab?.type === 'table_data' && !displayResult)" class="flex-center gap-2 text-secondary p-4">
        <Scaling :size="20" class="spin" />
        <span>{{ displayResult?.loading ? 'Executing query...' : 'Initializing data...' }}</span>
        <span v-if="displayResult?.loading" class="elapsed-pill">{{ formatElapsed(displayResult.elapsedMs) }}</span>
        <button
          v-if="displayResult?.loading && displayResult.execId"
          class="cancel-inline"
          title="Stop running query"
          @click="queryStore.cancelQuery(tabId)"
        >
          Stop
        </button>
      </div>

      <template v-else>
        <!-- RESULTS VIEW -->
        <div v-if="activePaneTab === 'results'" class="results-view">
          <div v-if="displayResult?.error" class="error-container flex-center flex-direction-column gap-3 overflow-auto">
            <template v-if="displayResult.error.includes('No database selected')">
              <Database :size="48" class="text-warning opacity-50" />
              <div class="error-header text-xl font-bold">No Database Selected</div>
              <div class="error-message text-center max-w-md opacity-80">
                This connection doesn't have a default database. You must either specify one in your connection settings or use a <code class="bg-white/10 px-1 rounded">USE database;</code> statement.
              </div>
              <div class="flex gap-3 mt-4">
                <button class="button-primary px-6" @click="handleEditConnection">
                  Set Default Database
                </button>
                <button class="button-secondary" @click="activePaneTab = 'messages'">View Details</button>
              </div>
            </template>
            <template v-else>
              <AlertCircle :size="48" class="text-error" />
              <div class="error-message">{{ displayResult.error }}</div>
              <button class="button-secondary" @click="activePaneTab = 'messages'">View Details</button>
            </template>
          </div>
          <DataGrid 
            v-else
            ref="dataGridRef"
            :tabId="tabId"
            :result="currentResult!"
          />
        </div>
        
        <!-- MESSAGES VIEW -->
        <div v-else-if="activePaneTab === 'messages'" class="messages-view">
          <div v-if="displayResult?.error" class="message error">
            <div class="flex items-center gap-2 mb-1">
              <AlertCircle :size="14" />
              <span class="font-bold">Error:</span>
            </div>
            <pre class="whitespace-pre-wrap pl-5">{{ displayResult.error }}</pre>
          </div>

          <div v-if="displayResult?.messages && displayResult.messages.length > 0" class="flex flex-col gap-1 mb-4">
             <div v-for="(msg, idx) in displayResult.messages" :key="idx" class="text-xs font-mono opacity-80 border-l-2 border-accent-primary pl-2 py-1 bg-tertiary/30">
                {{ msg }}
             </div>
          </div>
          
          <div v-if="displayResult?.results" class="flex flex-col gap-2">
             <div v-for="(res, idx) in displayResult.results" :key="idx" class="message-card p-3 rounded border border-border bg-tertiary">
                <div class="flex items-center gap-2 mb-2 font-mono text-xs text-secondary border-b border-border pb-1">
                   <Terminal :size="12" />
                   <span class="font-bold">Query {{ idx + 1 }}:</span>
                   <span class="truncate" :title="(res as any).query">{{ (res as any).query || 'Unknown query' }}</span>
                </div>
                <div class="flex items-center gap-4 text-xs">
                   <div v-if="res.rows && res.rows.length > 0" class="text-success">
                      {{ res.rows.length }} rows returned
                   </div>
                   <div v-else class="text-success">
                      {{ res.affectedRows }} rows affected
                   </div>
                   <div class="text-secondary opacity-70">
                      {{ (res.executionTimeMs / 1000).toFixed(3) }}s
                   </div>
                </div>
             </div>
          </div>
          
          <div v-else-if="!displayResult?.error" class="empty-state">No messages.</div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.result-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-top: 1px solid var(--border-color);
  background: var(--bg-secondary);
  position: relative;
}

.result-toolbar {
  height: 36px;
  padding: 0 8px;
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border-color);
}

.pane-tabs {
  height: 100%;
  gap: 4px;
}

.pane-tab {
  height: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  color: var(--text-secondary);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.pane-tab:hover {
  color: var(--text-primary);
  background: var(--glass-border);
}

.pane-tab.active {
  color: var(--accent-primary);
  border-bottom-color: var(--accent-primary);
}

.pane-content {
  flex: 1;
  overflow: hidden;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.results-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}


.messages-view {
  padding: 16px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  overflow: auto;
  flex: 1;
  min-height: 0;
}

.message {
  padding: 8px 12px;
  border-radius: 4px;
  margin-bottom: 8px;
}

.message.success {
  color: var(--text-success);
  background: var(--bg-success);
}

.message.error {
  color: var(--text-error);
  background: var(--bg-error);
}

.message.error pre {
  white-space: pre-wrap;
  word-wrap: break-word;
  overflow-wrap: break-word;
  word-break: break-all;
}


.error-container {
  height: 100%;
  padding: 40px;
  text-align: center;
}

.error-message {
  color: var(--text-error);
  font-weight: 500;
  max-width: 500px;
  word-break: break-all;
}

.flex-direction-column { flex-direction: column; }
.gap-3 { gap: 12px; }

.icon-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  display: flex;
}

.icon-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.text-accent {
  color: var(--accent-primary);
}

.empty-state {
  padding: 32px;
  text-align: center;
  color: var(--text-secondary);
  font-style: italic;
}

.h-full { height: 100%; }

.spin {
  animation: spin 1s linear infinite;
}

.elapsed-pill {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  font-family: var(--font-mono, monospace);
  font-variant-numeric: tabular-nums;
  font-size: 0.75rem;
  background: rgba(234, 179, 8, 0.1);
  color: rgb(234, 179, 8);
  border: 1px solid rgba(234, 179, 8, 0.25);
  border-radius: 10px;
  min-width: 60px;
  justify-content: center;
}

.cancel-inline {
  background: rgba(239, 68, 68, 0.12);
  color: rgb(239, 68, 68);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: 4px;
  padding: 2px 10px;
  font-size: 0.75rem;
  cursor: pointer;
}
.cancel-inline:hover {
  background: rgba(239, 68, 68, 0.22);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* Inspector Overlay */
.inspector-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(2px);
  z-index: 100;
  padding: 20px;
}

.inspector-card {
  width: 100%;
  max-width: 800px;
  background: var(--bg-secondary);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
}

.inspector-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-tertiary);
}

.inspector-title {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-secondary);
}

.inspector-body {
  flex: 1;
  padding: 16px;
}

.inspector-textarea {
  width: 100%;
  height: 400px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 12px;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  resize: none;
  outline: none;
}



.separator {
  width: 1px;
  height: 16px;
  background: var(--border-color);
  margin: 0 8px;
}

.inline-sub-tabs {
  display: flex;
  gap: 4px;
  align-items: center;
  overflow-x: auto;
  max-width: 300px; /* Prevent taking too much space */
  scrollbar-width: none;
}

.inline-sub-tabs::-webkit-scrollbar {
  display: none;
}

.result-chip {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  font-size: 0.7rem;
  font-weight: 600;
  border: 1px solid transparent;
  cursor: pointer;
  background: var(--bg-primary);
  color: var(--text-secondary);
  transition: all 0.2s;
}

.result-chip:hover {
  transform: scale(1.1);
}

.result-chip.active {
  box-shadow: 0 0 0 2px var(--bg-tertiary), 0 0 0 3px currentColor;
}

/* Color cycles */
.result-chip.color-0 { /* Green */
  color: #10b981;
  background: rgba(16, 185, 129, 0.1);
  border-color: rgba(16, 185, 129, 0.2);
}
.result-chip.color-0.active {
  background: #10b981;
  color: white;
}

.result-chip.color-1 { /* Yellow/Orange */
  color: #f59e0b;
  background: rgba(245, 158, 11, 0.1);
  border-color: rgba(245, 158, 11, 0.2);
}
.result-chip.color-1.active {
  background: #f59e0b;
  color: white;
}

.result-chip.color-2 { /* Blue */
  color: #3b82f6;
  background: rgba(59, 130, 246, 0.1);
  border-color: rgba(59, 130, 246, 0.2);
}
.result-chip.color-2.active {
  background: #3b82f6;
  color: white;
}

.result-chip.color-3 { /* Pink/Purple */
  color: #ec4899;
  background: rgba(236, 72, 153, 0.1);
  border-color: rgba(236, 72, 153, 0.2);
}
.result-chip.color-3.active {
  background: #ec4899;
  color: white;
}
</style>
