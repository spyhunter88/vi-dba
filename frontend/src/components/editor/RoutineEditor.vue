<script setup lang="ts">
import { ref, onMounted, computed, watch, onBeforeUnmount } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { Save, X, Database, Play, Info, Clock, Camera, Unplug, Link, RotateCcw } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';
import SnapshotManager from '../ui/SnapshotManager.vue';
import type { RoutineDefinition } from '../../types';
import * as monaco from 'monaco-editor';
import { registerSqlCompletionProvider } from '../../utils/sqlCompletion';
import { parseAndLintSql } from '../../utils/sqlLinter';
import { registerGlobalFormattingProvider } from '../../utils/sqlFormatter';

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const connectionStore = useConnectionStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();

const tab = computed(() => tabStore.tabs.find((t: any) => t.id === props.tabId));

const routineName = ref('');
const routineType = ref('');
const definition = ref('');
const catalog = ref<string | undefined>(undefined);
const schema = ref<string | undefined>(undefined);
const loading = ref(false);
const error = ref<string | null>(null);
const originalDefinition = ref<string>('');

const dbType = computed(() => {
  const conn = connectionStore.connections.find(c => c.id === tab.value?.connectionId);
  return conn?.dbType || 'postgreSQL';
});

const editorContainer = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let completionProvider: monaco.IDisposable | null = null;
const cachedSchema = ref<any[]>([]);
const showSnapshots = ref(false);
const showReconnectModal = ref(false);

watch(showSnapshots, () => {
  setTimeout(() => {
    if (editor) editor.layout();
  }, 10);
});

onMounted(async () => {
  console.log('RoutineEditor mounted for:', tab.value?.id);
  if (editorContainer.value) {
    editor = monaco.editor.create(editorContainer.value, {
      value: '',
      language: 'sql',
      automaticLayout: true,
      minimap: { enabled: false },
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      padding: { top: 16, bottom: 16 },
      scrollBeyondLastLine: false,
      theme: uiStore.resolvedTheme === 'light' ? 'vs' : 'vs-dark',
      wordWrap: 'on',
    });

    registerGlobalFormattingProvider();

    editor.onDidChangeModelContent(() => {
      if (loading.value) return;
      const value = editor?.getValue();
      definition.value = value || '';
      if (tab.value) {
        tab.value.content = definition.value;
        tab.value.isDirty = definition.value !== originalDefinition.value;
      }
      triggerLinter();
    });

    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      handleSave();
    });

    // Add command for Ctrl+Shift+S to save snapshot
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyS, () => {
      handleSaveSnapshot();
    });
    
    // Initial layout
    setTimeout(() => editor?.layout(), 100);

    loadSchema().then(() => {
      runLinter();
    });
    registerAutocomplete();
  }

  if (tab.value?.metadata?.name) {
    loading.value = true;
    try {
      const def = await invoke<RoutineDefinition>('get_routine_definition', {
        id: tab.value.connectionId,
        name: tab.value.metadata.name,
        routineType: tab.value.metadata.routineType,
        catalog: tab.value.metadata.catalog,
        schema: tab.value.metadata.schema
      });
      routineName.value = def.name;
      routineType.value = def.routineType;
      definition.value = def.definition;
      catalog.value = def.catalog;
      schema.value = def.schema;
      
      originalDefinition.value = def.definition;
      
      if (editor) {
        editor.setValue(def.definition);
        tab.value.isDirty = false;
      }
    } catch (e: any) {
      console.error('Failed to load routine:', e);
      error.value = e.toString();
    } finally {
      loading.value = false;
    }
  } else {
    // New routine fallback
    catalog.value = tab.value?.database || connectionStore.activeDatabase || undefined;
    schema.value = tab.value?.schema || connectionStore.activeSchema || undefined;
  }
});

onBeforeUnmount(() => {
  editor?.dispose();
  completionProvider?.dispose();
  clearTimeout(lintTimeout);
});

let lintTimeout: any = null;
function runLinter() {
  if (!editor) return;
  const model = editor.getModel();
  if (!model) return;
  const markers = parseAndLintSql(model, cachedSchema.value);
  monaco.editor.setModelMarkers(model, 'sql-linter', markers);
}

function triggerLinter() {
  clearTimeout(lintTimeout);
  lintTimeout = setTimeout(runLinter, 500);
}

watch(() => cachedSchema.value, () => {
  runLinter();
}, { deep: true });

function handleFormat() {
  if (!editor) return;
  editor.trigger('format', 'editor.action.formatDocument', null);
}

async function loadSchema() {
  if (!tab.value?.connectionId) return;
  try {
    const schemaJson = await invoke<string | null>('get_schema_cache', { id: tab.value.connectionId });
    if (schemaJson) {
      cachedSchema.value = JSON.parse(schemaJson);
    }
  } catch (e) {
    console.error('Failed to load schema cache:', e);
  }
}

function registerAutocomplete() {
  if (completionProvider) completionProvider.dispose();
  completionProvider = registerSqlCompletionProvider(
    () => cachedSchema.value,
    () => dbType.value,
    (model) => model === editor?.getModel()
  );
}

watch(() => tabStore.activeTabId, (newId) => {
  if (newId === props.tabId && editor) {
    setTimeout(() => editor?.layout(), 50);
  }
});

watch(() => uiStore.resolvedTheme, (newTheme) => {
  if (editor) {
    const monacoTheme = newTheme === 'light' ? 'vs' : 'vs-dark';
    monaco.editor.setTheme(monacoTheme);
  }
}, { immediate: true });

watch(() => tab.value?.content, (newContent) => {
  if (editor && newContent !== undefined && newContent !== editor.getValue()) {
    editor.setValue(newContent);
    definition.value = newContent;
  }
});

async function handleSave() {
  console.log('handleSave called');
  if (!tab.value) {
    console.error('handleSave: tab.value is undefined');
    return;
  }
  
  loading.value = true;
  error.value = null;

  try {
    const def: RoutineDefinition = {
      name: routineName.value,
      routineType: routineType.value,
      definition: definition.value,
      catalog: catalog.value,
      schema: schema.value
    };

    console.log('Saving routine...', def);
    await invoke('save_routine', {
      id: tab.value.connectionId,
      definition: def
    });

    console.log('Routine saved successfully');
    uiStore.showToast(`${routineType.value} "${routineName.value}" saved successfully.`);
    originalDefinition.value = definition.value;
    tab.value.isDirty = false;
    
    // Refresh objects in sidebar
    await connectionStore.refreshObjects(tab.value.connectionId);
    await handleSaveSnapshot(); // Auto-snapshot on save
  } catch (e: any) {
    error.value = e.toString();
    uiStore.showToast(`Failed to save routine: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

async function handleSaveSnapshot() {
  if (tab.value) {
    const content = editor?.getValue() || '';
    tab.value.content = content;
    definition.value = content;
    await sessionStore.saveSnapshot();
  }
}

function restoreSnapshot(content: string) {
  if (editor) {
    editor.setValue(content);
  }
}

function handleReconnect(connId: string) {
  if (tab.value) {
    tab.value.connectionId = connId;
    tab.value.isDetached = false;
    showReconnectModal.value = false;
    uiStore.showToast('Reconnected successfully.');
    loadSchema();
  }
}

function handleClose() {
  tabStore.closeTab(props.tabId);
}

const connectionName = computed(() => {
  const conn = connectionStore.connections.find((c: any) => c.id === tab.value?.connectionId);
  return conn?.name || 'No Connection';
});

const isConnected = computed(() => {
  const id = tab.value?.connectionId;
  return id ? connectionStore.connectedIds.has(id) : false;
});
</script>

<template>
  <div class="routine-editor flex-column h-full">
    <div class="editor-header flex-between glass">
      <div class="header-left flex-center gap-4">
        <div class="routine-info flex-center gap-2">
          <Play :size="16" class="text-accent" />
          <span class="text-xs font-bold">{{ routineName }}</span>
          <span class="text-xs text-secondary bg-surface px-2 py-0.5 rounded">{{ routineType }}</span>
        </div>
        <div class="connection-info flex-center gap-2" v-if="!tab?.isDetached">
          <Database :size="14" class="text-accent" />
          <div class="flex flex-col">
            <span class="text-xs font-bold text-accent leading-tight">{{ connectionName }}</span>
            <span v-if="catalog || schema" class="text-9px opacity-70 text-accent leading-none">
              {{ catalog || schema }}{{ catalog && schema ? '.' + schema : '' }}
            </span>
          </div>
          <template v-if="!isConnected">
            <span class="text-xs text-muted-foreground italic">(Disconnected)</span>
            <button class="button-primary sm h-6 px-2 text-xs" @click="tab?.connectionId && connectionStore.connect(tab.connectionId)">
              Connect
            </button>
          </template>
        </div>
        <button v-else class="button-warning sm flex-center gap-2" @click="showReconnectModal = true">
           <Unplug :size="14" class="mr-2" />
           <span>Detached (Reconnect)</span>
        </button>
        <div class="save-info flex-center gap-1 text-xs text-secondary opacity-70 ml-2">
          <Info :size="12" class="mr-2" />
          <span> Saves by dropping and recreating.</span>
        </div>
      </div>
      <div class="header-right flex-center gap-2">
        <button class="icon-btn" title="Save Snapshot" @click="handleSaveSnapshot">
          <Camera :size="14" />
        </button>
        <button class="icon-btn" title="Toggle Snapshots" @click="showSnapshots = !showSnapshots">
          <Clock :size="14" />
        </button>
        <button class="button-secondary sm" @click="handleFormat" title="Format SQL (Alt+Shift+F)">
          <RotateCcw :size="14" />
          Format
        </button>
        <button class="button-primary sm" @click="handleSave" :disabled="loading || !tab?.isDirty">
          <div v-if="loading" class="spinner-xs mr-2"></div>
          <Save v-else :size="14" />
          {{ loading ? 'Saving...' : 'Save' }}
        </button>
        <button class="button-secondary sm" @click="handleClose">
          <X :size="14" />
          Close
        </button>
      </div>
    </div>

    <div v-if="error" class="error-banner flex-between">
      <span>{{ error }}</span>
      <button class="icon-btn xs" @click="error = null"><X :size="14" /></button>
    </div>

    <div class="editor-content flex flex-1 relative flex-row overflow-hidden">
      <div v-if="loading" class="loading-overlay flex-center">
        <div class="spinner"></div>
        <span class="ml-2">Processing...</span>
      </div>
      <div ref="editorContainer" class="monaco-wrapper"></div>
      <SnapshotManager 
        :show="showSnapshots" 
        :tab-id="props.tabId" 
        @close="showSnapshots = false" 
        @restore="restoreSnapshot"
      />

      <!-- Reconnect Modal -->
      <div v-if="showReconnectModal" class="absolute inset-0 z-50 flex-center glass-overlay">
         <div class="glass-panel p-4 flex flex-col gap-4 min-w-[300px]">
            <h3 class="font-bold text-lg">Reconnect Tab</h3>
            <p class="text-sm text-secondary">Select a connection to attach this tab to:</p>
            <div class="flex flex-col gap-2 max-h-[300px] overflow-y-auto">
               <button 
                  v-for="conn in connectionStore.connections" 
                  :key="conn.id"
                  class="connection-item button-secondary text-left flex items-center gap-2 p-2"
                  @click="handleReconnect(conn.id)"
               >  
                  <Link :size="14" />
                  {{ conn.name }}
               </button>
               <div v-if="connectionStore.connections.length === 0" class="text-xs text-center p-2 opacity-50">
                  No connections available.
               </div>
            </div>
            <button class="button-ghost sm self-end" @click="showReconnectModal = false">Cancel</button>
         </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.routine-editor {
  background: var(--bg-primary);
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.editor-header {
  height: 40px;
  padding: 0 12px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.monaco-wrapper {
  flex: 1;
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.error-banner {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
  padding: 8px 20px;
  border-bottom: 1px solid rgba(239, 68, 68, 0.2);
  font-size: 0.85rem;
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.3);
  z-index: 10;
  backdrop-filter: blur(2px);
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--glass-border);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.ml-2 { margin-left: 8px; }

.save-info {
  border-left: 1px solid var(--border-color);
  padding-left: 12px;
}

.sm {
  padding: 4px 12px;
  font-size: 0.8rem;
}

.button-warning {
  background: rgba(234, 179, 8, 0.1);
  color: rgb(234, 179, 8);
  border: 1px solid rgba(234, 179, 8, 0.2);
  border-radius: 4px;
  cursor: pointer;
}
.button-warning:hover {
  background: rgba(234, 179, 8, 0.2);
}


.glass-overlay {
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
}

.glass-panel {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.3);
}

.min-w-\[300px\] { min-width: 300px; }
.overflow-y-auto { overflow-y: auto; }
.text-left { text-align: left; }
.items-center { align-items: center; }
.self-end { align-self: flex-end; }
.connection-item:hover {
  background: var(--bg-tertiary);
}
</style>
