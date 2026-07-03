<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  Send, Sparkles, Loader2, History, X, Square, Clock, ArrowDownToLine, ArrowUpFromLine, Database,
  ClipboardCopy, ClipboardPaste, Scissors, Play as PlayIcon
} from 'lucide-vue-next';
import ContextMenu from './ContextMenu.vue';

export interface AiCallStats {
  tokensIn: number;
  tokensOut: number;
  timeMs: number;
  model: string;
  tableCount: number;
}

const props = defineProps<{
  modelValue: string | undefined;
  loading: boolean;
  history?: string[];
  models?: string[];
  selectedModel?: string;
  aiMode?: string;
  elapsedMs?: number;
  lastStats?: AiCallStats | null;
}>();

const emit = defineEmits(['update:modelValue', 'update:selectedModel', 'send', 'select-history', 'cancel']);

const modelValueLocal = computed({
  get: () => props.modelValue || '',
  set: (val: string) => emit('update:modelValue', val)
});

function handleSend() {
  const val = props.modelValue || '';
  if (val.trim() && !props.loading) {
    emit('send', val);
  }
}

function handleCancel() {
  emit('cancel');
}

const showHistory = ref(false);

const elapsedSeconds = computed(() => ((props.elapsedMs || 0) / 1000).toFixed(1));

function selectHistory(item: string) {
  emit('update:modelValue', item);
  showHistory.value = false;
}

const textareaRows = computed(() => {
  const lines = (props.modelValue || '').split('\n').length;
  return Math.min(10, Math.max(1, lines));
});

const contextMenu = ref({
  show: false,
  x: 0,
  y: 0,
  items: [] as any[]
});

const textareaRef = ref<HTMLTextAreaElement | null>(null);

async function handleContextMenu(e: MouseEvent) {
  e.preventDefault();
  const textarea = textareaRef.value;
  if (!textarea) return;

  const start = textarea.selectionStart;
  const end = textarea.selectionEnd;
  const hasSelection = start !== end;
  
  const items = [];

  if (hasSelection) {
    items.push({
      label: 'Cut',
      icon: Scissors,
      shortcut: 'Ctrl+X',
      action: () => {
        textarea.focus();
        document.execCommand('cut');
      }
    });
    items.push({
      label: 'Copy',
      icon: ClipboardCopy,
      shortcut: 'Ctrl+C',
      action: () => {
        textarea.focus();
        document.execCommand('copy');
      }
    });
    items.push({
      label: 'Send to AI',
      icon: PlayIcon,
      shortcut: 'Ctrl+Enter',
      action: () => handleSend()
    });
  } else {
    let clipboardText = '';
    let isClipboardEmpty = true;
    try {
      clipboardText = await navigator.clipboard.readText();
      isClipboardEmpty = !clipboardText;
    } catch (err) {
      console.warn('Clipboard read failed:', err);
      isClipboardEmpty = false;
    }

    items.push({
      label: 'Paste',
      icon: ClipboardPaste,
      shortcut: 'Ctrl+V',
      disabled: isClipboardEmpty,
      action: async () => {
        textarea.focus();
        if (!clipboardText) {
          try { clipboardText = await navigator.clipboard.readText(); } catch(e) {}
        }
        if (clipboardText) {
          const start = textarea.selectionStart;
          const end = textarea.selectionEnd;
          const text = textarea.value;
          const newValue = text.substring(0, start) + clipboardText + text.substring(end);
          emit('update:modelValue', newValue);
          
          // Set cursor position after paste
          setTimeout(() => {
            textarea.selectionStart = textarea.selectionEnd = start + clipboardText.length;
          }, 0);
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
</script>

<template>
  <div class="ai-prompt-pane glass-panel">
    <div class="prompt-input-container">
      <div class="prompt-icon">
        <Sparkles v-if="!loading" :size="18" class="text-accent animate-pulse-slow" />
        <Loader2 v-else :size="18" class="text-accent animate-spin" />
      </div>
      <textarea 
        ref="textareaRef"
        v-model="modelValueLocal"
        class="prompt-textarea" 
        :placeholder="loading ? 'AI is thinking...' : 'Describe what you want to achieve... (e.g., List all users who signed up in the last 30 days)'"
        @keydown.enter.ctrl="handleSend"
        @contextmenu="handleContextMenu"
        :disabled="loading"
        :rows="textareaRows"
      ></textarea>
      
      <div v-if="models && models.length > 0 && !loading && aiMode !== 'integrated'" class="model-selector-wrapper">
        <select 
          :value="selectedModel" 
          @change="$emit('update:selectedModel', ($event.target as HTMLSelectElement).value)"
          class="model-select"
          :class="{ 'required-highlight': !selectedModel }"
        >
          <option value="">Select Model...</option>
          <option v-for="m in models" :key="m" :value="m">{{ m }}</option>
        </select>
      </div>

      <div class="prompt-actions">
        <button 
          v-if="history && history.length > 0 && !loading"
          class="action-icon-btn" 
          @click="showHistory = !showHistory" 
          title="Prompt History"
        >
          <History :size="16" />
        </button>
        
        <button 
          v-if="loading"
          class="cancel-btn" 
          @click="handleCancel"
          title="Cancel Generation"
        >
          <Square :size="14" fill="currentColor" />
        </button>
        
        <button 
          v-else
          class="send-btn" 
          :disabled="!(modelValue || '').trim() || loading" 
          @click="handleSend"
        >
          <Send :size="16" />
        </button>
      </div>
    </div>

    <div class="prompt-bottom-actions flex-between px-3 py-1.5 border-t border-white/5 bg-black/10">
      <div class="stats-bar flex gap-3 text-[10px] font-mono">
        <template v-if="loading">
          <span class="flex-center gap-1 text-accent">
            <Loader2 :size="11" class="animate-spin" /> Generating… {{ elapsedSeconds }}s
          </span>
        </template>
        <template v-else-if="lastStats">
          <span class="stat-chip flex-center gap-1" title="Round-trip time">
            <Clock :size="11" /> {{ (lastStats.timeMs / 1000).toFixed(2) }}s
          </span>
          <span class="stat-chip flex-center gap-1" title="Input tokens (schema + prompt)">
            <ArrowUpFromLine :size="11" /> {{ lastStats.tokensIn.toLocaleString() }} in
          </span>
          <span class="stat-chip flex-center gap-1" title="Output tokens (generated SQL)">
            <ArrowDownToLine :size="11" /> {{ lastStats.tokensOut.toLocaleString() }} out
          </span>
          <span class="stat-chip flex-center gap-1" title="Tables/views sent in schema context">
            <Database :size="11" /> {{ lastStats.tableCount }} tables
          </span>
          <span v-if="lastStats.model" class="stat-chip opacity-50 truncate max-w-[140px]" :title="lastStats.model">
            {{ lastStats.model }}
          </span>
        </template>
        <template v-else>
          <span class="opacity-40 italic">Full schema sent on each request.</span>
        </template>
      </div>
      <div class="flex gap-2 text-[10px] opacity-40 font-mono italic whitespace-nowrap">
        Ctrl + Enter to send
      </div>
    </div>

    <!-- History Popover -->
    <div v-if="showHistory" class="history-popover glass shadow-xl">
      <div class="flex-between p-2 border-b border-white/10">
        <span class="text-xs font-bold opacity-50 uppercase tracking-wider">Recent Prompts</span>
        <button class="icon-btn xs" @click="showHistory = false"><X :size="12" /></button>
      </div>
      <div class="history-list">
        <div 
          v-for="(item, idx) in history" 
          :key="idx" 
          class="history-item" 
          @click="selectHistory(item)"
        >
          {{ item }}
        </div>
      </div>
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
.ai-prompt-pane {
  padding: 12px;
  background: var(--bg-secondary);
  border: 1px solid var(--glass-border);
  box-shadow: 0 4px 12px rgba(0,0,0,0.1);
  margin-bottom: 12px;
  position: relative;
  overflow: visible !important;
}

.prompt-input-container {
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 8px 12px;
  transition: border-color 0.2s;
}

.prompt-input-container:focus-within {
  border-color: var(--accent-primary);
}

.prompt-icon {
  display: flex;
  align-items: center;
}

.prompt-textarea {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: 0.95rem;
  resize: none;
  padding: 4px 0;
  max-height: 400px;
  outline: none;
  font-family: inherit;
  line-height: 1.5;
}

.model-selector-wrapper {
  margin-left: 8px;
  padding-left: 8px;
  border-left: 1px solid var(--border-color);
}

.model-select {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 0.75rem;
  outline: none;
  cursor: pointer;
}

.model-select.required-highlight {
  color: #fbbf24;
  font-weight: bold;
}

.action-icon-btn {
  background: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-muted);
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
}

.action-icon-btn:hover {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border-color: var(--accent-primary);
}

.prompt-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.send-btn, .cancel-btn {
  border: none;
  border-radius: 6px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform 0.1s, opacity 0.2s;
}

.send-btn {
  background: var(--accent-primary);
  color: white;
}

.cancel-btn {
  background: #ef4444;
  color: white;
}

.send-btn:hover:not(:disabled), .cancel-btn:hover {
  transform: scale(1.05);
  opacity: 0.9;
}

.send-btn:disabled {
  background: var(--bg-tertiary);
  color: var(--text-muted);
  cursor: not-allowed;
}

.history-popover {
  position: absolute;
  top: 100%;
  right: 12px;
  width: 300px;
  max-height: 200px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  z-index: 100;
  margin-top: 8px;
  display: flex;
  flex-direction: column;
}

.history-list {
  overflow-y: auto;
  padding: 4px;
}

.history-item {
  padding: 8px 12px;
  font-size: 0.85rem;
  cursor: pointer;
  border-radius: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.history-item:hover {
  background: var(--bg-tertiary);
}

.animate-pulse-slow {
  animation: pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

.table-selector-popover {
  position: absolute;
  top: 100%;
  left: 0;
  width: 480px;
  max-height: 550px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  z-index: 1000;
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  box-shadow: 0 25px 60px -12px rgba(0,0,0,0.6);
  overflow: hidden;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
}

.close-btn:hover {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.table-list {
  flex: 1;
  overflow-y: auto;
  min-height: 200px;
  max-height: 350px;
  background: rgba(0,0,0,0.1);
}

.table-item {
  border-bottom: 1px solid rgba(255,255,255,0.03);
}

.table-item:last-child {
  border-bottom: none;
}

.search-input:focus {
  background: rgba(255,255,255,0.08);
  border-color: var(--accent-primary);
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}

.animate-pulse-slow {
  animation: pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

.prompt-bottom-actions {
  min-height: 32px;
}

.stats-bar {
  align-items: center;
  flex-wrap: wrap;
  color: var(--text-secondary);
}

.stat-chip {
  padding: 1px 6px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  white-space: nowrap;
}

.action-btn {
  background: transparent;
  border: none;
  color: var(--text-primary);
  cursor: pointer;
  padding: 2px 4px;
}

.action-btn:hover {
  color: var(--accent-primary);
}

.action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.3 !important;
}

.flex-between { display: flex; justify-content: space-between; align-items: center; }
.flex-center { display: flex; align-items: center; justify-content: center; }
.gap-1\.5 { gap: 6px; }
.py-1\.5 { padding-top: 6px; padding-bottom: 6px; }
.bg-black\/10 { background-color: rgba(0,0,0,0.1); }

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
  transform: translateY(10px);
}

</style>
