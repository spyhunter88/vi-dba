<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useConnectionStore } from '../../stores/connections';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import AiPromptPane from '../ui/AiPromptPane.vue';
import SqlEditor from './SqlEditor.vue';
import ResultPane from './ResultPane.vue';

const props = defineProps<{
  tabId: string;
}>();

const connectionStore = useConnectionStore();
const tabStore = useTabStore();
const queryStore = useQueryStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();

const tab = computed(() => tabStore.tabs.find((t: any) => t.id === props.tabId));

interface AiSqlResult {
  sql: string;
  tokensIn: number;
  tokensOut: number;
  timeMs: number;
  model: string;
  tableCount: number;
}

const aiLoading = ref(false);
const aiPrompt = ref('');
const promptHistory = ref<string[]>([]);
const availableModels = ref<string[]>([]);
const selectedModel = ref('');
const lastStats = ref<AiSqlResult | null>(null);

// Live elapsed-time ticker shown while a generation is in flight.
const elapsedMs = ref(0);
let timerId: ReturnType<typeof setInterval> | null = null;
function startTimer() {
  elapsedMs.value = 0;
  const t0 = Date.now();
  timerId = setInterval(() => { elapsedMs.value = Date.now() - t0; }, 100);
}
function stopTimer() {
  if (timerId) { clearInterval(timerId); timerId = null; }
}

async function handleAiPrompt(promptText: string) {
  if (!tab.value?.connectionId) return;

  const aiMode = sessionStore.aiMode;
  if (aiMode === 'builtin' && availableModels.value.length > 0 && !selectedModel.value) {
    uiStore.showToast('Please select a model first', 'error');
    return;
  }

  aiLoading.value = true;
  startTimer();
  queryStore.addTabMessage(props.tabId, `Starting AI generation (${aiMode}) with prompt: "${promptText}"...`);

  try {
    // Add current content as context if it exists
    let augmentedPrompt = promptText;
    if (tab.value.content) {
      const lastPrompt = promptHistory.value[0] || 'the previous query';
      augmentedPrompt = `In the context of ${lastPrompt} and the current SQL below:\n\n${tab.value.content}\n\nUpdate it as follows: ${promptText}`;
      queryStore.addTabMessage(props.tabId, "Adding editor content to context...");
    }

    queryStore.addTabMessage(props.tabId, "Invoking AI model...");
    const res = await invoke<AiSqlResult>('generate_ai_sql', {
       connectionId: tab.value!.connectionId,
       humanInput: augmentedPrompt,
       modelName: aiMode === 'builtin' ? selectedModel.value : null,
       database: tab.value.database,
       schema: tab.value.schema
    });

    // Directly apply to editor
    const timestamp = new Date().toLocaleTimeString();
    const comment = `-- AI Generated at ${timestamp}:\n`;
    tab.value.content = comment + res.sql;

    lastStats.value = res;
    promptHistory.value.unshift(promptText);
    if (promptHistory.value.length > 20) promptHistory.value.pop();

    queryStore.addTabMessage(
      props.tabId,
      `SQL generated and applied. ${res.tokensIn} tokens in / ${res.tokensOut} out · ${(res.timeMs / 1000).toFixed(2)}s · ${res.tableCount} tables (${res.model}).`
    );
    aiPrompt.value = '';
  } catch (e: any) {
    queryStore.addTabMessage(props.tabId, `Error: ${e}`);
    uiStore.showToast('AI Generation: ' + e, 'error');
  } finally {
    stopTimer();
    aiLoading.value = false;
  }
}

async function handleCancel() {
  try {
    await invoke('cancel_ai_generation');
    queryStore.addTabMessage(props.tabId, "Cancellation requested...");
  } catch (e) {
    console.error("Failed to cancel AI generation:", e);
  }
}


const splitPercent = ref(70);
const isResizing = ref(false);

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
  const container = document.querySelector('.ai-sql-editor');
  if (!container) return;
  
  const rect = container.getBoundingClientRect();
  const y = e.clientY - rect.top;
  const h = rect.height;
  
  splitPercent.value = Math.max(20, Math.min(80, (y / h) * 100));
}

let unlistenProgress: (() => void) | null = null;
onMounted(async () => {
  unlistenProgress = await listen<string>('ai-progress', (event) => {
    queryStore.addTabMessage(props.tabId, event.payload);
  });

  sessionStore.fetchAppSettings();

  if (tab.value?.connectionId && !connectionStore.connectionObjects[tab.value.connectionId]) {
    connectionStore.refreshObjects(tab.value.connectionId);
  }

  try {
    const models = await invoke<string[]>('list_local_models');
    availableModels.value = models;
  } catch (e) {
    console.error("Failed to list models:", e);
  }
});

onBeforeUnmount(() => {
  if (unlistenProgress) unlistenProgress();
  stopTimer();
});
</script>

<template>
  <div v-if="tab" class="ai-sql-editor h-full flex flex-col">
    <div class="ai-controls-area p-4 bg-surface/50 border-b border-white/5">
      <AiPromptPane 
        v-model="aiPrompt"
        v-model:selectedModel="selectedModel"
        :loading="aiLoading" 
        :history="promptHistory"
        :models="availableModels"
        :ai-mode="sessionStore.aiMode"
        :elapsed-ms="elapsedMs"
        :last-stats="lastStats"
        @send="handleAiPrompt"
        @cancel="handleCancel"
      />

    </div>

    <!-- Main Editor & Result Area -->
    <div class="editor-workspace flex-1 flex flex-col min-h-0">
      <div class="editor-section" :style="{ height: splitPercent + '%' }">
        <SqlEditor v-model="tab.content" :tab-id="tabId" />
      </div>
      
      <div class="splitter" @mousedown="startResizing">
        <div class="splitter-handle"></div>
      </div>

      <div class="result-section flex-1 min-h-0 flex flex-col" :style="{ height: (100 - splitPercent) + '%' }">
        <ResultPane :tab-id="tabId" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-sql-editor {
  background: var(--bg-primary);
}

.ai-controls-area {
  position: relative;
  z-index: 50; /* Ensure popovers overlay the workspace */
}

.ai-review-box {
  border-left: 4px solid var(--accent-primary);
  background: rgba(var(--accent-rgb), 0.05);
}

.generated-preview {
  font-family: 'JetBrains Mono', monospace;
  color: var(--text-primary);
  max-height: 150px;
}

.editor-workspace {
  position: relative;
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

/* Transitions */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease-out;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-20px);
}
</style>
