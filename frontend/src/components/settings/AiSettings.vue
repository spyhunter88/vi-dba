<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { invoke } from '@tauri-apps/api/core';
import { Sparkles, Save, Info, ExternalLink, Loader2, FolderOpen, Check } from 'lucide-vue-next';

const uiStore = useUiStore();
const sessionStore = useSessionStore();

const settings = ref({
  aiMode: 'integrated',
  ollamaUrl: 'http://localhost:11434',
  ollamaModel: 'qwen2.5:7b',
  cloudProvider: 'gemini',
  cloudApiKey: '',
  cloudModel: 'gemini-1.5-flash',
  cloudBaseUrl: '',
  aiExcludePatterns: '',
});

const DEFAULT_MODELS: Record<string, string> = {
  gemini: 'gemini-1.5-flash',
  openai: 'gpt-4o-mini',
  claude: 'claude-haiku-4-5-20251001',
};

const API_KEY_LINKS: Record<string, string> = {
  gemini: 'https://aistudio.google.com/app/apikey',
  openai: 'https://platform.openai.com/api-keys',
  claude: 'https://console.anthropic.com/settings/keys',
};

function onProviderChange() {
  settings.value.cloudModel = DEFAULT_MODELS[settings.value.cloudProvider] ?? '';
  settings.value.cloudBaseUrl = '';
}

const availableModels = ref<string[]>([]);
const loading = ref(true);
const saving = ref(false);
const testingOllama = ref(false);
const ollamaStatus = ref<{ success: boolean; message: string; models: string[] } | null>(null);

onMounted(async () => {
  try {
    const s: any = await invoke('get_app_settings');
    if (s) {
      if (s.aiMode) settings.value.aiMode = s.aiMode;
      if (s.ollamaUrl) settings.value.ollamaUrl = s.ollamaUrl;
      if (s.ollamaModel) settings.value.ollamaModel = s.ollamaModel;
      if (s.cloudProvider) settings.value.cloudProvider = s.cloudProvider;
      if (s.cloudApiKey) settings.value.cloudApiKey = s.cloudApiKey;
      if (s.cloudModel) settings.value.cloudModel = s.cloudModel;
      if (s.cloudBaseUrl) settings.value.cloudBaseUrl = s.cloudBaseUrl;
      if (s.aiExcludePatterns) settings.value.aiExcludePatterns = s.aiExcludePatterns;
    }
    await fetchModels();
  } catch (e) {
    console.error('Failed to load settings:', e);
  } finally {
    loading.value = false;
  }
});

async function fetchModels() {
  try {
    const models = await invoke<string[]>('list_local_models');
    availableModels.value = models;
  } catch (e) {
    console.error('Failed to fetch models:', e);
  }
}

async function handleSave() {
  try {
    saving.value = true;
    // Merge into current settings so unrelated fields (history retention, etc.) are preserved.
    const current: any = (await invoke('get_app_settings')) || {};
    await invoke('update_app_settings', { settings: { ...current, ...settings.value } });
    await sessionStore.fetchAppSettings();
    uiStore.showToast('AI Settings saved successfully.');
  } catch (e) {
    uiStore.showToast('Failed to save AI settings: ' + e, 'error');
  } finally {
    saving.value = false;
  }
}

async function testOllama() {
  try {
    testingOllama.value = true;
    ollamaStatus.value = null;
    const models = await invoke<string[]>('test_ollama_connection', { url: settings.value.ollamaUrl });
    ollamaStatus.value = { 
      success: true, 
      message: 'Successfully connected to Ollama!', 
      models 
    };
    if (models.length > 0 && !settings.value.ollamaModel) {
      settings.value.ollamaModel = models[0]!;
    }
  } catch (e: any) {
    ollamaStatus.value = { 
      success: false, 
      message: e.toString(), 
      models: [] 
    };
  } finally {
    testingOllama.value = false;
  }
}
async function openModelsFolder() {
  try {
    await invoke('open_models_directory');
  } catch (e) {
    uiStore.showToast('Failed to open models folder: ' + e, 'error');
  }
}
</script>

<template>
  <div class="settings-content p-6 max-w-2xl mx-auto">
    <div class="header flex-between mb-8">
      <div class="flex-center gap-3">
        <div class="p-2 bg-accent-10 rounded-lg">
          <Sparkles class="text-accent" :size="24" />
        </div>
        <div>
          <h2 class="text-xl font-bold">AI Assistant Settings</h2>
          <p class="text-sm text-secondary">Configure your AI model and connection details.</p>
        </div>
      </div>
      <button class="button-primary" @click="handleSave" :disabled="saving">
        <Loader2 v-if="saving" class="animate-spin mr-2" :size="16" />
        <Save v-else class="mr-2" :size="16" />
        Save Settings
      </button>
    </div>

    <div v-if="loading" class="flex-center p-12">
      <Loader2 class="animate-spin text-accent" :size="32" />
    </div>

    <div v-else class="space-y-8">
      <!-- AI Mode Selection -->
      <section class="settings-section">
        <label class="section-label">AI Inference Mode</label>
        <div class="grid grid-cols-3 gap-4 mt-3">
          <button 
            class="mode-card" 
            :class="{ active: settings.aiMode === 'builtin' }"
            @click="settings.aiMode = 'builtin'"
          >
            <span class="font-bold">Builtin</span>
            <span class="text-xs opacity-50 text-center">Local model (Privacy First)</span>
          </button>
          <button 
            class="mode-card" 
            :class="{ active: settings.aiMode === 'integrated' }"
            @click="settings.aiMode = 'integrated'"
          >
            <span class="font-bold">Integrated</span>
            <span class="text-xs opacity-50 text-center">Ollama (Server)</span>
          </button>
          <button 
            class="mode-card" 
            :class="{ active: settings.aiMode === 'cloud' }"
            @click="settings.aiMode = 'cloud'"
          >
            <span class="font-bold">Cloud</span>
            <span class="text-xs opacity-50 text-center">External APIs</span>
          </button>
        </div>
      </section>

      <!-- Schema Context Reduction -->
      <section class="settings-section">
        <label class="section-label">Schema Context</label>
        <div class="space-y-4 mt-3 p-4 bg-surface-30 rounded-lg border border-white/5">
          <div class="p-3 bg-blue-500-10 border border-blue-500-20 rounded-lg flex gap-3 text-xs">
            <Info :size="16" class="shrink-0 text-blue-400 mr-2" />
            <div class="space-y-1">
              <span class="text-blue-100 leading-relaxed">
                The <strong>full database schema</strong> is sent to the model on every request (single call).
                To reduce input tokens, exclude tables you never query against — backups, soft-delete staging, audit copies, etc.
              </span>
            </div>
          </div>
          <div class="input-group">
            <label>Exclude Table Patterns <span class="opacity-50">(comma or newline separated)</span></label>
            <textarea
              v-model="settings.aiExcludePatterns"
              rows="3"
              placeholder="backup, *_bak, pre_delete*, tmp_, audit_log"
              class="exclude-textarea"
            ></textarea>
            <span class="text-10px opacity-50">
              Matching is case-insensitive. Use <code class="bg-black-30 px-1 rounded">prefix*</code> /
              <code class="bg-black-30 px-1 rounded">*suffix</code> for edge wildcards; bare text matches anywhere in the name.
            </span>
          </div>
        </div>
      </section>

      <!-- Builtin Guidelines -->
      <section v-if="settings.aiMode === 'builtin'" class="settings-section anim-slide-up">
        <div class="flex-between">
          <label class="section-label">Builtin (Local) Setup Guide</label>
          <button class="button-secondary text-xs text-accent flex-center gap-1 hover:underline bg-transparent border-none cursor-pointer" @click="openModelsFolder">
            <FolderOpen :size="12" /> Open Models Folder
          </button>
        </div>
        <div class="space-y-4 mt-3 p-4 bg-surface-30 rounded-lg border border-white/5 text-sm">
          <div class="p-3 bg-blue-500-10 border border-blue-500-20 rounded-lg flex gap-3 text-xs mb-4">
            <Info :size="16" class="shrink-0 text-blue-400 mr-2" />
            <div class="space-y-1">
              <span class="text-blue-100 font-bold leading-relaxed">Builtin mode runs entirely on your machine. No data leaves your computer.</span>
            </div>
          </div>

          <p class="font-medium text-accent">How to add models:</p>
          <ol class="list-decimal list-inside space-y-3 opacity-90">
            <li>
              Download a <strong>GGUF</strong> model (e.g., <a href="https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF" target="_blank" class="underline hover:text-accent">Qwen2.5-Instruct</a>).
            </li>
            <li>
              Place the <code class="bg-black-30 px-1 rounded">.gguf</code> file in the models folder.
              <p class="text-10px opacity-60 ml-5 mt-1">You can have multiple models; you'll be able to choose between them in the AI Editor.</p>
            </li>
            <li>
              <strong>Critical:</strong> Place a <code class="bg-black-30 px-1 rounded">tokenizer.json</code> file (matching your model) in the <strong>same folder</strong>.
            </li>
            <li>
              Restart the app (or just open/re-open the AI Editor) to see your new models.
            </li>
          </ol>

          <div v-if="availableModels.length > 0" class="model-list-mini mt-4 p-3 bg-black/20 rounded border border-white/5">
            <div class="flex-between mb-2">
              <span class="text-10px uppercase font-bold opacity-50">Detected Models ({{ availableModels.length }})</span>
              <button class="text-10px button-secondary text-accent hover:underline bg-transparent border-none cursor-pointer" @click="fetchModels">Reload</button>
            </div>
            <div class="flex flex-wrap gap-2">
              <span v-for="m in availableModels" :key="m" class="px-2 py-0.5 bg-accent/10 border border-accent/20 rounded text-10px">{{ m }}</span>
            </div>
          </div>
          <div v-else class="mt-4 p-3 bg-red-500/5 rounded border border-red-500/20 text-center">
            <p class="text-10px text-red-400">No models detected. Please add GGUF files to the folder above.</p>
            <button class="text-10px text-accent hover:underline bg-transparent border-none cursor-pointer mt-1" @click="fetchModels">Check Again</button>
          </div>

          <button class="button-secondary w-full mt-4 flex-center gap-2" @click="openModelsFolder">
            <FolderOpen :size="16" /> Manage Local Models
          </button>
        </div>
      </section>

      <!-- Ollama Settings -->
      <section v-if="settings.aiMode === 'integrated'" class="settings-section anim-slide-up">
        <div class="flex-between">
          <label class="section-label">Ollama Configuration</label>
          <a href="https://ollama.com/" target="_blank" class="text-xs text-accent flex-center gap-1 hover:underline">
            Get Ollama <ExternalLink :size="10" />
          </a>
        </div>
        <div class="space-y-4 mt-3 p-4 bg-surface-30 rounded-lg border border-white/5">
          <div class="input-group">
            <label>API URL</label>
            <div class="flex gap-2">
              <input v-model="settings.ollamaUrl" placeholder="http://localhost:11434" class="flex-1" />
              <button 
                class="button-secondary sm flex-center gap-1.5 min-w-[120px]" 
                @click="testOllama" 
                :disabled="testingOllama"
              >
                <Loader2 v-if="testingOllama" :size="14" class="animate-spin" />
                <Check v-else :size="14" />
                Test
              </button>
            </div>
          </div>
          
          <div v-if="ollamaStatus" class="p-3 rounded border text-xs anim-slide-up" :class="ollamaStatus.success ? 'bg-green-500/10 border-green-500/20 text-green-400' : 'bg-red-500/10 border-red-500/20 text-red-400'">
            <p class="font-bold flex items-center gap-2">
              <Info :size="14" /> {{ ollamaStatus.message }}
            </p>
            <div v-if="ollamaStatus.models.length > 0" class="mt-2 flex flex-wrap gap-1">
              <span class="opacity-70 mr-1">Available Models:</span>
              <span v-for="m in ollamaStatus.models" :key="m" class="px-1.5 py-0.5 bg-black/20 rounded border border-white/5 opacity-80">{{ m }}</span>
            </div>
          </div>

          <div class="input-group">
            <label>Model Name</label>
            <input v-model="settings.ollamaModel" placeholder="qwen2.5:7b" />
          </div>
          <div class="p-4 bg-accent-5 border border-accent-10 rounded-lg space-y-2 text-xs text-accent/80 shadow-md">
            <p class="font-bold flex-center gap-2 text-accent"><Info :size="14" /> Setup Guide:</p>
            <ul class="list-disc list-inside space-y-1 opacity-90 font-medium">
              <li>Install Ollama from <a href="https://ollama.com" target="_blank" class="underline hover:text-accent">ollama.com</a></li>
              <li>Run command: <code class="bg-black-30 px-1 rounded text-white">ollama pull {{ settings.ollamaModel || 'qwen2.5:7b' }}</code></li>
              <li>Ensure Ollama is running before using the AI Editor</li>
            </ul>
          </div>
        </div>
      </section>

      <!-- Cloud Settings -->
      <section v-if="settings.aiMode === 'cloud'" class="settings-section anim-slide-up">
        <label class="section-label">Cloud API Configuration</label>
        <div class="space-y-4 mt-3 p-4 bg-surface-30 rounded-lg border border-white/5">
          <div class="input-group">
            <label>Provider</label>
            <select v-model="settings.cloudProvider" @change="onProviderChange">
              <option value="gemini">Google Gemini</option>
              <option value="openai">OpenAI</option>
              <option value="claude">Anthropic Claude</option>
            </select>
          </div>

          <div class="input-group">
            <label>API Key</label>
            <div class="flex gap-2">
              <input v-model="settings.cloudApiKey" type="password" placeholder="Enter your API key" class="flex-1" />
              <a
                v-if="API_KEY_LINKS[settings.cloudProvider]"
                :href="API_KEY_LINKS[settings.cloudProvider]"
                target="_blank"
                class="button-secondary sm flex-center"
                title="Get API Key"
              >
                <ExternalLink :size="14" />
              </a>
            </div>
          </div>

          <div class="input-group">
            <label>Model</label>
            <input v-model="settings.cloudModel" :placeholder="DEFAULT_MODELS[settings.cloudProvider] ?? 'model-name'" />
            <span class="text-10px opacity-50">
              <template v-if="settings.cloudProvider === 'gemini'">e.g. gemini-1.5-flash, gemini-2.0-flash</template>
              <template v-else-if="settings.cloudProvider === 'openai'">e.g. gpt-4o-mini, gpt-4o</template>
              <template v-else-if="settings.cloudProvider === 'claude'">e.g. claude-haiku-4-5-20251001, claude-sonnet-4-6</template>
            </span>
          </div>

          <!-- Base URL: only for OpenAI-compatible endpoints -->
          <div v-if="settings.cloudProvider === 'openai'" class="input-group">
            <label>Base URL <span class="opacity-50">(optional — for Azure or custom endpoints)</span></label>
            <input v-model="settings.cloudBaseUrl" placeholder="https://api.openai.com" />
          </div>

          <div class="p-3 bg-red-500-10 border border-red-500-20 rounded-lg text-xs text-red-100 flex gap-3 shadow-md">
            <Info :size="16" class="shrink-0 text-red-400" />
            <div class="space-y-1">
              <p class="font-bold text-red-400 uppercase tracking-wider">Privacy Note:</p>
              <p class="text-red-400 leading-relaxed">In Cloud Mode, your schema names and prompts are sent to the provider. Actual data values are <strong>never</strong> transmitted.</p>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.settings-section {
  display: flex;
  flex-direction: column;
}

.section-label {
  font-size: 0.85rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

.mode-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s;
  gap: 4px;
}

.mode-card:hover:not(:disabled):not(.active) {
  border-color: var(--accent-primary);
  background: rgba(var(--accent-rgb), 0.1);
}

.mode-card.active:hover {
  filter: brightness(1.1);
}

.mode-card.active {
  background: var(--accent-primary);
  border-color: var(--accent-primary);
  color: white;
}

.mode-card:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.input-group label {
  font-size: 0.8rem;
  font-weight: 500;
  opacity: 0.8;
}

input, select {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 8px 12px;
  color: var(--text-primary);
  font-size: 0.9rem;
  outline: none;
}

input:focus, select:focus {
  border-color: var(--accent-primary);
}

.exclude-textarea {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 8px 12px;
  color: var(--text-primary);
  font-size: 0.85rem;
  font-family: 'JetBrains Mono', monospace;
  outline: none;
  resize: vertical;
  width: 100%;
}

.exclude-textarea:focus {
  border-color: var(--accent-primary);
}

.mx-auto { margin-left: auto; margin-right: auto; }
.max-w-2xl { max-width: 42rem; }
.space-y-8 > * + * { margin-top: 2rem; }
.space-y-4 > * + * { margin-top: 1rem; }
.space-y-3 > * + * { margin-top: 0.75rem; }
.space-y-2 > * + * { margin-top: 0.5rem; }
.space-y-1 > * + * { margin-top: 0.25rem; }
.grid { display: grid; }
.grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.gap-4 { gap: 1rem; }
.mt-3 { margin-top: 0.75rem; }
.mb-8 { margin-bottom: 2rem; }
.p-6 { padding: 1.5rem; }
.p-3 { padding: 0.75rem; }
.p-2 { padding: 0.5rem; }
.mr-2 { margin-right: 0.5rem; }
.ml-5 { margin-left:1.25rem; }
.mt-1 { margin-top: 0.25rem; }
.mt-4 { margin-top: 1rem; }
.mb-4 { margin-bottom: 1rem; }
.gap-3 { gap: 0.75rem; }
.w-full { width: 100%; }
.font-bold { font-weight: 700; }
.text-xl { font-size: 1.25rem; }
.anim-slide-up {
  animation: slideUp 0.3s ease-out;
}

@keyframes slideUp {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.bg-surface-30 { background-color: rgba(255, 255, 255, 0.03); }
.bg-black-30 { background-color: rgba(0, 0, 0, 0.3); }
.shrink-0 { flex-shrink: 0; }
.text-red-100 { color: #fee2e2; }
.text-red-200 { color: #fecaca; }
.text-red-200-90 { color: rgba(254, 202, 202, 0.9); }
.text-red-400 { color: #f87171; }
.bg-red-500-10 { background-color: rgba(239, 68, 68, 0.18); }
.border-red-500-20 { border-color: rgba(239, 68, 68, 0.5); }
.bg-blue-500-10 { background-color: rgba(59, 130, 246, 0.15); }
.border-blue-500-20 { border-color: rgba(59, 130, 246, 0.4); }
.text-blue-100 { color: #2563eb; }
.text-blue-400 { color: #3b82f6; }
.bg-accent-5 { background-color: rgba(var(--accent-rgb), 0.05); }
.border-accent-10 { border-color: rgba(var(--accent-rgb), 0.2); }
</style>
