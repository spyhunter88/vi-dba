<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useUiStore } from '../../stores/ui';
import { useConnectionStore } from '../../stores/connections';
import { useSessionStore } from '../../stores/session';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import AiPromptPane from '../ui/AiPromptPane.vue';
import RoutineEditor from './RoutineEditor.vue';

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const queryStore = useQueryStore();
const uiStore = useUiStore();
const connectionStore = useConnectionStore();
const sessionStore = useSessionStore();

const tab = computed(() => tabStore.tabs.find(t => t.id === props.tabId));

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

async function handleCancel() {
  try {
    await invoke('cancel_ai_generation');
    queryStore.addTabMessage(props.tabId, "Cancellation requested...");
  } catch (e) {
    console.error("Failed to cancel AI generation:", e);
  }
}

async function handleAiPrompt(promptText: string) {
  if (!tab.value?.connectionId) return;

  if (availableModels.value.length > 0 && !selectedModel.value) {
    uiStore.showToast('Please select a model first', 'error');
    return;
  }

  aiLoading.value = true;
  startTimer();
  queryStore.addTabMessage(props.tabId, `Starting AI generation with prompt: "${promptText}"...`);

  try {
    let augmentedPrompt = promptText;
    if (tab.value.content) {
      const lastPrompt = promptHistory.value[0] || 'the previous query';
      augmentedPrompt = `In the context of ${lastPrompt} and the current routine definition below:\n\n${tab.value.content}\n\nUpdate it as follows: ${promptText}`;
      queryStore.addTabMessage(props.tabId, "Adding routine definition to context...");
    }

    queryStore.addTabMessage(props.tabId, "Invoking AI model...");
    const aiMode = sessionStore.aiMode;
    const res = await invoke<AiSqlResult>('generate_ai_sql', {
       connectionId: tab.value!.connectionId,
       humanInput: augmentedPrompt,
       modelName: aiMode === 'builtin' ? selectedModel.value : null,
       database: tab.value.database,
       schema: tab.value.schema
    });

    // Directly apply to editor
    const timestamp = new Date().toLocaleTimeString();
    const comment = `/* AI Generated at ${timestamp}: */\n`;
    tab.value.content = comment + res.sql;

    lastStats.value = res;
    promptHistory.value.unshift(promptText);
    if (promptHistory.value.length > 20) promptHistory.value.pop();

    queryStore.addTabMessage(
      props.tabId,
      `Routine generated. ${res.tokensIn} tokens in / ${res.tokensOut} out · ${(res.timeMs / 1000).toFixed(2)}s · ${res.tableCount} tables (${res.model}).`
    );
    // Clear prompt ONLY on success
    aiPrompt.value = '';
  } catch (e: any) {
    queryStore.addTabMessage(props.tabId, `Error: ${e}`);
    uiStore.showToast('AI Generation: ' + e, 'error');
  } finally {
    stopTimer();
    aiLoading.value = false;
  }
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
  <div v-if="tab" class="ai-routine-editor h-full flex flex-col">
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

    <div class="editor-workspace flex-1 min-h-0">
      <RoutineEditor :tab-id="tabId" />
    </div>
  </div>
</template>

<style scoped>
.ai-routine-editor {
  background: var(--bg-primary);
}

.ai-controls-area {
  position: relative;
  z-index: 50;
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
