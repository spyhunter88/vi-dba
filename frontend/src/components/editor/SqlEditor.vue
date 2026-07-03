<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed, nextTick } from 'vue';
import * as monaco from 'monaco-editor';
import { 
  Play, Square, RotateCcw, Save, Search, Database, FileText, Clock, 
  Camera, Unplug, Sparkles, History, ClipboardCopy, ClipboardPaste, Scissors,
  BookOpen
} from 'lucide-vue-next';
import { useConnectionStore } from '../../stores/connections';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { invoke } from '@tauri-apps/api/core';
import SnapshotManager from '../ui/SnapshotManager.vue';
import QueryHistoryManager from '../ui/QueryHistoryManager.vue';
import SqlHelpManager from '../ui/SqlHelpManager.vue';
import { registerSqlCompletionProvider } from '../../utils/sqlCompletion';
import { parseAndLintSql } from '../../utils/sqlLinter';
import { registerGlobalFormattingProvider } from '../../utils/sqlFormatter';
import ContextMenu from '../ui/ContextMenu.vue';

const props = defineProps<{
  modelValue: string | undefined;
  tabId: string;
}>();

const emit = defineEmits(['update:modelValue', 'execute']);

const connectionStore = useConnectionStore();
const tabStore = useTabStore();
const queryStore = useQueryStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();

const editorContainer = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let completionProvider: monaco.IDisposable | null = null;
const cachedSchema = ref<any[]>([]);
const showSnapshots = ref(false);
const showHistory = ref(false);
const showHelp = ref(false);
const contextMenu = ref({
  show: false,
  x: 0,
  y: 0,
  items: [] as any[]
});

watch([showSnapshots, showHistory, showHelp], () => {
  setTimeout(() => {
    if (editor) editor.layout();
  }, 10);
});

const currentTab = computed(() => tabStore.tabs.find(t => t.id === props.tabId));

const currentConnection = computed(() => {
  const connId = currentTab.value?.connectionId;
  if (!connId) return null;
  return connectionStore.connections.find(c => c.id === connId) || null;
});
const dbType = computed(() => currentConnection.value?.dbType || 'postgreSQL');
const currentConnectionId = computed(() => currentTab.value?.connectionId || null);

const tabResult = computed(() => queryStore.queryResults[props.tabId]);
const isRunning = computed(() => !!tabResult.value?.loading);
const elapsedLabel = computed(() => formatElapsed(tabResult.value?.elapsedMs ?? tabResult.value?.totalTimeMs));

function formatElapsed(ms?: number): string {
  if (ms === undefined || ms === null) return '';
  if (ms < 1000) return `${ms} ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(2)} s`;
  const m = Math.floor(sec / 60);
  const s = (sec - m * 60).toFixed(1);
  return `${m}m ${s}s`;
}

onMounted(() => {
  if (editorContainer.value) {
    editor = monaco.editor.create(editorContainer.value, {
      value: props.modelValue || '',
      language: 'sql',
      theme: uiStore.resolvedTheme === 'light' ? 'vs' : 'vs-dark',
      automaticLayout: true,
      minimap: { enabled: false },
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      padding: { top: 16, bottom: 16 },
      scrollBeyondLastLine: false,
      contextmenu: false,
      wordWrap: 'on',
    });

    registerGlobalFormattingProvider();

    editor.onDidChangeModelContent(() => {
      const value = editor?.getValue();
      emit('update:modelValue', value);
      if (currentTab.value && value !== props.modelValue) {
        currentTab.value.isDirty = true;
      }
      triggerLinter();
    });

    // Add action for Ctrl+R to execute all
    editor.addAction({
      id: 'execute-all',
      label: 'Execute All',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyR],
      run: () => handleExecute('all')
    });

    // Add action for Ctrl+Shift+R to execute selection
    editor.addAction({
      id: 'execute-selection',
      label: 'Execute Selection',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyR],
      run: () => handleExecute('selection')
    });

    // Add action for F5 to execute auto (election or all)
    editor.addAction({
      id: 'execute-auto',
      label: 'Execute',
      keybindings: [monaco.KeyCode.F5],
      run: () => handleExecute('auto')
    });

    // Add command for Ctrl+S to save
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      handleSave();
    });

    // Add command for Ctrl+Shift+S to save snapshot (Placeholder for now)
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyS, () => {
      uiStore.showToast('Snapshot saved (simulated)');
    });

    // Add command for Ctrl+H to toggle SQL Help Center panel
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyH, () => {
      showHelp.value = !showHelp.value;
      if (showHelp.value) {
        showSnapshots.value = false;
        showHistory.value = false;
      }
    });

    loadSchema().then(() => {
      runLinter();
    });
    registerAutocomplete();

    // Listen for paste event from history popup
    const pasteListener = (e: any) => {
      if (editor && e.detail?.query) {
        handlePaste(e.detail.query);
      }
    };

    const handlePaste = (text: string) => {
      const selection = editor?.getSelection();
      if (selection) {
        editor?.executeEdits('history-paste', [
          {
            range: selection,
            text,
            forceMoveMarkers: true
          }
        ]);
      }
    };

    window.addEventListener('vi-paste-query', pasteListener);

    // Cross-window paste from separate history-detail window
    let unlistenRequest: any = null;
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('vi-paste-query-request', (event: any) => {
        console.log('[SqlEditor] Received paste request:', event.payload);
        handlePaste(event.payload.query);
      });
    }).then(u => unlistenRequest = u);

    onBeforeUnmount(() => {
      window.removeEventListener('vi-paste-query', pasteListener);
      if (unlistenRequest) unlistenRequest();
      clearTimeout(lintTimeout);
    });

    // Custom context menu handler
    editorContainer.value.addEventListener('contextmenu', handleContextMenu);
  }
});

async function handleContextMenu(e: MouseEvent) {
  if (!editor) return;
  e.preventDefault();

  const selection = editor.getSelection();
  const hasSelection = selection && !selection.isEmpty();
  const items = [];

  if (hasSelection) {
    items.push({
      label: 'Cut',
      icon: Scissors,
      shortcut: 'Ctrl+X',
      action: () => {
        editor?.focus();
        document.execCommand('cut');
      }
    });
    items.push({
      label: 'Copy',
      icon: ClipboardCopy,
      shortcut: 'Ctrl+C',
      action: () => {
        editor?.focus();
        document.execCommand('copy');
      }
    });
    items.push({
      label: 'Execute Selection',
      icon: Play,
      shortcut: 'Ctrl+Shift+R',
      action: () => handleExecute('selection')
    });
  } else {
    let clipboardText = '';
    let isClipboardEmpty = true;
    try {
      // Try to check clipboard
      clipboardText = await navigator.clipboard.readText();
      isClipboardEmpty = !clipboardText;
    } catch (err) {
      console.warn('Clipboard read failed:', err);
      // Fallback: assume not empty if permission denied
      isClipboardEmpty = false;
    }

    items.push({
      label: 'Paste',
      icon: ClipboardPaste,
      shortcut: 'Ctrl+V',
      disabled: isClipboardEmpty,
      action: async () => {
        editor?.focus();
        if (!clipboardText) {
          try { clipboardText = await navigator.clipboard.readText(); } catch(e) {}
        }
        if (clipboardText) {
          const selection = editor?.getSelection();
          if (selection) {
            editor?.executeEdits('context-menu-paste', [{
              range: selection,
              text: clipboardText,
              forceMoveMarkers: true
            }]);
          }
        } else {
          document.execCommand('paste');
        }
      }
    });
  }

  contextMenu.value = {
    show: true,
    x: e.clientX,
    y: e.clientY,
    items
  };
}

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
  if (!currentTab.value?.connectionId) return;
  try {
    const schemaJson = await invoke<string | null>('get_schema_cache', { id: currentTab.value.connectionId });
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

function insertSnippet(code: string) {
  if (!editor) return;
  editor.focus();
  const contribution = editor.getContribution('snippetController2') as any;
  if (contribution && typeof contribution.insert === 'function') {
    contribution.insert(code);
  } else {
    const selection = editor.getSelection();
    if (selection) {
      const range = new monaco.Range(
        selection.startLineNumber,
        selection.startColumn,
        selection.endLineNumber,
        selection.endColumn
      );
      editor.executeEdits('insert-snippet', [
        {
          range,
          text: code,
          forceMoveMarkers: true,
        },
      ]);
    }
  }
}

onBeforeUnmount(() => {
  editor?.dispose();
  completionProvider?.dispose();
});

watch(() => props.modelValue, (newVal) => {
  if (editor && newVal !== editor.getValue()) {
    editor.setValue(newVal || '');
  }
});

watch(() => uiStore.resolvedTheme, (newTheme) => {
  if (editor) {
    const monacoTheme = newTheme === 'light' ? 'vs' : 'vs-dark';
    monaco.editor.setTheme(monacoTheme);
  }
}, { immediate: true });

// When this tab becomes active again after being hidden by v-show, Monaco needs
// an explicit layout() call because ResizeObserver doesn't fire on display:none→block.
watch(() => tabStore.activeTabId, (newId) => {
  if (newId === props.tabId && editor) {
    nextTick(() => editor?.layout());
  }
});

async function handleExecute(mode: 'all' | 'selection' | 'auto' = 'auto') {
  if (!props.tabId || !editor) return;
  
  const query = editor.getValue();
  emit('update:modelValue', query);
  
  if (currentTab.value) currentTab.value.content = query;
  
  let selectedText = undefined;
  
  if (mode === 'all') {
    selectedText = undefined;
  } else if (mode === 'selection') {
    const selection = editor.getSelection();
    if (selection && !selection.isEmpty()) {
       selectedText = editor.getModel()?.getValueInRange(selection);
    } else {
       return;
    }
  } else {
    const selection = editor.getSelection();
    if (selection && !selection.isEmpty()) {
       selectedText = editor.getModel()?.getValueInRange(selection);
    }
  }
  
  await queryStore.executeQueryInTab(props.tabId, selectedText);
}

async function handleSave() {
  if (!currentTab.value || !editor) return;

  const content = editor.getValue();
  let path = currentTab.value.filePath;
  let name = currentTab.value.title;

  if (!path) {
    const inputName = prompt('Enter a name for this script:', name.endsWith('.sql') ? name : `${name}.sql`);
    if (!inputName) return;
    name = inputName;
    
    try {
      const scriptInfo: any = await invoke('save_script', {
        connectionId: currentTab.value.connectionId,
        name,
        content,
        database: currentTab.value.database,
        schema: currentTab.value.schema
      });
      
      currentTab.value.filePath = scriptInfo.path;
      currentTab.value.title = scriptInfo.name;
      currentTab.value.isDirty = false;
      uiStore.showToast(`Script "${name}" saved successfully.`);
    } catch (e) {
      console.error('Failed to save script:', e);
      uiStore.showToast('Failed to save script: ' + e, 'error');
    }
  } else {
    try {
      await invoke('save_script', {
        connectionId: currentTab.value.connectionId,
        name: currentTab.value.title,
        content,
        database: currentTab.value.database,
        schema: currentTab.value.schema
      });
      currentTab.value.isDirty = false;
      uiStore.showToast(`Script "${currentTab.value.title}" saved successfully.`);
    } catch (e) {
      console.error('Failed to save script:', e);
      uiStore.showToast('Failed to save script: ' + e, 'error');
    }
  }
}

const connectionName = computed(() => {
  const conn = connectionStore.connections.find(c => c.id === currentTab.value?.connectionId);
  return conn?.name || 'No Connection';
});

const isConnected = computed(() => {
  const id = currentTab.value?.connectionId;
  return id ? connectionStore.connectedIds.has(id) : false;
});

function toggleDetail() {
  if (currentTab.value) {
    currentTab.value.showDetail = !currentTab.value.showDetail;
  }
}

function restoreSnapshot(content: string) {
  if (editor) {
    editor.setValue(content);
  }
}
</script>

<template>
  <div class="sql-editor-container">
    <div class="editor-toolbar flex-between glass">
      <div class="toolbar-left flex-center gap-4">
        <div class="flex-center gap-2">
          <button v-if="!isRunning" class="button-primary sm" @click="handleExecute()">
            <Play :size="14" />
            Run
          </button>
          <button
            v-else
            class="button-stop sm"
            title="Stop running query"
            @click="queryStore.cancelQuery(props.tabId)"
          >
            <Square :size="14" />
            Stop
          </button>
          <div v-if="isRunning || elapsedLabel" class="query-timer" :class="{ running: isRunning }" :title="isRunning ? 'Elapsed time' : 'Last query duration'">
            <Clock :size="12" />
            <span>{{ elapsedLabel || '0 ms' }}</span>
          </div>
          <button class="button-ai-sparkle sm" @click="tabStore.openAiSqlEditor(currentTab?.connectionId || '', currentTab?.database, currentTab?.schema)">
            <Sparkles :size="14" />
            Ask AI
          </button>
          <button class="button-secondary sm" @click="handleFormat">
            <RotateCcw :size="14" />
            Format
          </button>
        </div>
        <div class="connection-info flex-center gap-2" v-if="!currentTab?.isDetached">
          <Database :size="14" class="text-accent" />
          <div class="flex flex-col">
            <span class="text-xs font-bold text-accent leading-tight">{{ connectionName }}</span>
            <span v-if="currentTab?.database || currentTab?.schema" class="text-9px opacity-70 text-accent leading-none">
              {{ [currentTab.database, currentTab.schema].filter(Boolean).join('.') }}
            </span>
          </div>
          <template v-if="!isConnected">
            <span class="text-xs text-muted-foreground italic">(Disconnected)</span>
            <button class="button-primary sm h-6 px-2 text-xs" @click="currentTab?.connectionId && connectionStore.connect(currentTab.connectionId)">
              Connect
            </button>
          </template>
        </div>
        <button v-else class="button-warning sm flex-center gap-2" @click="tabStore.openReconnectModal(currentTab!)">
           <Unplug :size="14" />
           <span>Detached (Reconnect)</span>
        </button>
        <button class="button-secondary sm" @click="toggleDetail">
          <FileText :size="14" />
          Text
        </button>
      </div>
      <div class="toolbar-right flex-center gap-2">
        <button class="icon-btn" title="Search">
          <Search :size="14" />
        </button>
        <button class="icon-btn" title="Execute Selection (Ctrl+Shift+R)" @click="handleExecute('selection')">
          <Play :size="14" class="text-secondary" />
        </button>
        <div class="div-divider"></div>
        <button class="icon-btn" title="Save Snapshot" @click="sessionStore.saveSnapshot()">
          <Camera :size="14" />
        </button>
        <button class="icon-btn" title="Toggle Snapshots" @click="showSnapshots = !showSnapshots; showHistory = false; showHelp = false">
          <Clock :size="14" />
        </button>
        <button class="icon-btn" title="Query History" @click="showHistory = !showHistory; showSnapshots = false; showHelp = false">
          <History :size="14" />
        </button>
        <button class="icon-btn" :class="{ 'active': showHelp }" title="SQL Help Center (Ctrl+H)" @click="showHelp = !showHelp; showSnapshots = false; showHistory = false">
          <BookOpen :size="14" />
        </button>
        <button class="icon-btn" title="Save Script" @click="handleSave" :class="{ 'text-accent': currentTab?.isDirty }">
          <Save :size="14" />
        </button>
      </div>
    </div>
    <div class="editor-main flex-row flex-1 overflow-hidden">
      <div ref="editorContainer" class="monaco-wrapper"></div>
      <SnapshotManager 
        :show="showSnapshots" 
        :tab-id="props.tabId" 
        @close="showSnapshots = false" 
        @restore="restoreSnapshot"
      />
      <QueryHistoryManager 
        :show="showHistory" 
        :tab-id="props.tabId" 
        @close="showHistory = false" 
        @restore="restoreSnapshot"
      />
      <SqlHelpManager
        :show="showHelp"
        :tab-id="props.tabId"
        :db-type="dbType"
        :connection-id="currentConnectionId"
        :catalog="currentTab?.database"
        :schema="currentTab?.schema"
        :query-text="modelValue || ''"
        @close="showHelp = false"
        @insert="insertSnippet"
      />
      
      <!-- Reconnect Modal will be handled by Workspace globally -->
    </div>

    <ContextMenu 
      :show="contextMenu.show"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :items="contextMenu.items"
      @close="contextMenu.show = false"
    />
  </div>
</template>

<style scoped>
.sql-editor-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.editor-toolbar {
  height: 40px;
  padding: 0 12px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
  flex-shrink: 0;
}

.editor-main {
  display: flex;
  flex-direction: row;
}

.monaco-wrapper {
  flex: 1;
  width: 100%;
  overflow: hidden;
  min-width: 0;
}

.sm {
  padding: 4px 12px;
  font-size: 0.8rem;
}

.gap-2 {
  gap: 8px;
}

.gap-4 {
  gap: 16px;
}

.text-xs {
  font-size: 0.75rem;
}

.font-semibold {
  font-weight: 600;
}

.icon-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 6px;
  border-radius: 4px;
  display: flex;
}

.icon-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.icon-btn.active {
  background: var(--glass-border);
  color: var(--accent-primary);
}

.div-divider {
  width: 1px;
  height: 20px;
  background: var(--border-color);
  margin: 0 4px;
}

.button-warning {
  background: rgba(234, 179, 8, 0.1); /* yellow-500/10 */
  color: rgb(234, 179, 8);
  border: 1px solid rgba(234, 179, 8, 0.2);
  border-radius: 4px;
  cursor: pointer;
}
.button-warning:hover {
  background: rgba(234, 179, 8, 0.2);
}

.button-stop {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  font-size: 0.8rem;
  background: rgba(239, 68, 68, 0.12); /* red-500/12 */
  color: rgb(239, 68, 68);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s;
}
.button-stop:hover {
  background: rgba(239, 68, 68, 0.22);
}

.query-timer {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  font-size: 0.75rem;
  font-family: var(--font-mono, monospace);
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  min-width: 70px;
  justify-content: center;
}
.query-timer.running {
  color: rgb(234, 179, 8);
  border-color: rgba(234, 179, 8, 0.35);
  background: rgba(234, 179, 8, 0.08);
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

.absolute { position: absolute; }
.inset-0 { top: 0; right: 0; bottom: 0; left: 0; }
.z-50 { z-index: 50; }
.flex-col { flex-direction: column; }
.p-4 { padding: 1rem; }
.min-w-\[300px\] { min-width: 300px; }
.text-secondary { color: var(--text-secondary); }
.overflow-y-auto { overflow-y: auto; }
.text-left { text-align: left; }
.items-center { align-items: center; }
.self-end { align-self: flex-end; }

.connection-item:hover {
  background: var(--bg-tertiary);
}

</style>
