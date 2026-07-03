<script setup lang="ts">
import { ref, reactive, watch, computed } from 'vue';
import { useUiStore } from '../../stores/ui';
import { useConnectionStore } from '../../stores/connections';
import { X, Database, Globe, HardDrive, Key, Activity, Layers, Tag } from 'lucide-vue-next';
import type { DbType, ConnectionConfig } from '../../types';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  show: boolean;
  initialType: DbType | null;
}>();

const emit = defineEmits(['close']);

const uiStore = useUiStore();
const connectionStore = useConnectionStore();
const isTesting = ref(false);
const isSaving = ref(false);
const testResult = ref<{ success: boolean; message: string } | null>(null);
const saveError = ref<string | null>(null);

const form = reactive<ConnectionConfig>({
  id: '',
  name: '',
  dbType: 'mySQL',
  host: '127.0.0.1',
  port: 3306,
  user: 'root',
  password: '',
  database: '',
  group: '',
  environment: '',
});

const PRESET_ENVS = ['dev', 'test', 'beta', 'prod'] as const;

const existingGroups = computed(() => {
  const seen = new Set<string>();
  for (const c of connectionStore.connections) {
    if (c.group) seen.add(c.group);
  }
  return Array.from(seen).sort();
});

watch(() => props.show, (show) => {
  if (show) {
    if (connectionStore.editingConnection) {
      Object.assign(form, connectionStore.editingConnection);
      if (!form.group) form.group = '';
      if (!form.environment) form.environment = '';
    } else {
      form.id = '';
      form.name = '';
      form.dbType = props.initialType || 'mySQL';
      form.host = form.dbType === 'sqlite' ? '' : '127.0.0.1';
      form.port = form.dbType === 'postgreSQL' ? 5432 :
                  form.dbType === 'sqlServer' ? 1433 :
                  form.dbType === 'oracle' ? 1521 :
                  form.dbType === 'mongoDB' ? 27017 : 3306;
      form.user = form.dbType === 'sqlite' ? '' :
                  form.dbType === 'oracle' ? 'system' : 'root';
      form.password = '';
      form.database = '';
      form.group = '';
      form.environment = '';
    }
    testResult.value = null;
    saveError.value = null;
  }
});

async function handleTest() {
  isTesting.value = true;
  testResult.value = null;
  try {
    await invoke('test_connection', { config: form });
    testResult.value = { success: true, message: 'Connection successful!' };
  } catch (e: any) {
    testResult.value = { success: false, message: e.toString() };
  } finally {
    isTesting.value = false;
  }
}

async function handleSave() {
  if (isSaving.value) return;
  isSaving.value = true;
  saveError.value = null;
  
  try {
    if (connectionStore.editingConnection) {
      await connectionStore.updateConnection({ ...form });
    } else {
      form.id = Math.random().toString(36).substring(7);
      await connectionStore.addConnection({ ...form });
    }
    uiStore.showToast(`Connection "${form.name}" saved successfully.`);
    emit('close');
  } catch (e: any) {
    saveError.value = e.toString();
  } finally {
    isSaving.value = false;
  }
}
</script>

<template>
  <div v-if="show" class="modal-overlay flex-center">
    <div class="modal-content glass">
      <div class="modal-header flex-between">
        <div class="flex-center gap-2">
          <Database :size="20" class="text-accent" />
          <h3>{{ connectionStore.editingConnection ? 'Edit' : 'New' }} {{ form.dbType }} Connection</h3>
        </div>
        <button class="icon-btn" @click="emit('close')">
          <X :size="20" />
        </button>
      </div>

      <div class="modal-body">
        <div class="form-group">
          <label>Connection Name</label>
          <div class="input-wrapper">
            <input v-model="form.name" placeholder="Production Database" />
          </div>
        </div>

        <div class="form-row">
          <div class="form-group flex-1">
            <label><Layers :size="13" /> Project / Group</label>
            <div class="input-wrapper">
              <input
                v-model="form.group"
                placeholder="e.g. MyApp, ShopDB"
                :list="'group-list-' + form.id"
                autocomplete="off"
              />
              <datalist :id="'group-list-' + form.id">
                <option v-for="g in existingGroups" :key="g" :value="g" />
              </datalist>
            </div>
          </div>
          <div class="form-group" style="min-width:160px">
            <label><Tag :size="13" /> Environment</label>
            <div class="env-field">
              <div class="env-presets">
                <button
                  v-for="e in PRESET_ENVS"
                  :key="e"
                  type="button"
                  class="env-preset-btn"
                  :class="[e, { active: form.environment === e }]"
                  @click="form.environment = form.environment === e ? '' : e"
                >{{ e }}</button>
              </div>
              <div class="input-wrapper">
                <input v-model="form.environment" placeholder="custom (staging, qa…)" />
              </div>
            </div>
          </div>
        </div>

        <div class="form-row">
          <div class="form-group flex-1">
            <label><Globe :size="14" /> Host</label>
            <div class="input-wrapper">
              <input v-model="form.host" placeholder="localhost" />
            </div>
          </div>
          <div class="form-group w-100">
            <label>Port</label>
            <div class="input-wrapper">
              <input v-model.number="form.port" type="number" />
            </div>
          </div>
        </div>

        <div class="form-row">
          <div class="form-group flex-1">
            <label><HardDrive :size="14" /> User</label>
            <div class="input-wrapper">
              <input v-model="form.user" />
            </div>
          </div>
          <div class="form-group flex-1">
            <label><Key :size="14" /> Password</label>
            <div class="input-wrapper">
              <input v-model="form.password" type="password" />
            </div>
          </div>
        </div>

        <div class="form-group">
          <label>
            <Database :size="14" /> 
            {{ form.dbType === 'oracle' ? 'Service Name / SID' : form.dbType === 'mongoDB' ? 'Auth Database (Optional)' : 'Database (Default)' }}
          </label>
          <div class="input-wrapper">
            <input v-model="form.database" :placeholder="form.dbType === 'oracle' ? 'XE' : 'optional'" />
          </div>
        </div>

        <div v-if="testResult" class="test-result" :class="{ success: testResult.success }">
          {{ testResult.message }}
        </div>
        <div v-if="saveError" class="test-result flex gap-2">
          <span class="text-error">{{ saveError }}</span>
        </div>
      </div>

      <div class="modal-footer flex-between">
        <button class="button-secondary" @click="handleTest" :disabled="isTesting || isSaving">
          <Activity v-if="isTesting" :size="14" class="spin" />
          Test Connection
        </button>
        <div class="flex-center gap-2">
          <button class="button-secondary" @click="emit('close')">Cancel</button>
          <button class="button-primary" @click="handleSave" :disabled="!form.name || !form.host || isSaving">
            <Activity v-if="isSaving" :size="14" class="spin" />
            {{ connectionStore.editingConnection ? 'Update' : 'Save' }} Connection
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: var(--overlay-bg);
  backdrop-filter: blur(4px);
  z-index: 1000;
}

.modal-content {
  width: 100%;
  max-width: 500px;
  background: var(--bg-secondary);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-xl);
  animation: modal-in 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes modal-in {
  from { transform: scale(0.95); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

.modal-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);
}

.modal-body {
  padding: 24px 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-row {
  display: flex;
  gap: 16px;
}

label {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 6px;
}

.input-wrapper input {
  width: 100%;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 10px 12px;
  border-radius: 6px;
  outline: none;
  font-family: inherit;
  transition: border-color 0.2s;
}

.input-wrapper input:focus {
  border-color: var(--accent-primary);
}

.test-result {
  padding: 12px;
  border-radius: 6px;
  font-size: 0.85rem;
  background: var(--bg-error);
  color: var(--text-error);
  border: 1px solid var(--border-error);
}

.test-result.success {
  background: var(--bg-success);
  color: var(--text-success);
  border: 1px solid var(--border-success);
}

.modal-footer {
  padding: 16px 20px;
  background: var(--bg-tertiary);
  border-top: 1px solid var(--border-color);
}

.flex-1 { flex: 1; }
.w-100 { width: 100px; }
.gap-2 { gap: 8px; }
.text-error { color: var(--text-error); }

.env-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.env-presets {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.env-preset-btn {
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 0.7rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
}
.env-preset-btn:hover { opacity: 0.85; }
.env-preset-btn.dev         { border-color: rgba(59,130,246,0.3); }
.env-preset-btn.dev.active  { background: rgba(59,130,246,0.2); color: #60a5fa; border-color: #60a5fa; }
.env-preset-btn.test        { border-color: rgba(234,179,8,0.3); }
.env-preset-btn.test.active { background: rgba(234,179,8,0.2); color: #fbbf24; border-color: #fbbf24; }
.env-preset-btn.beta        { border-color: rgba(139,92,246,0.3); }
.env-preset-btn.beta.active { background: rgba(139,92,246,0.2); color: #a78bfa; border-color: #a78bfa; }
.env-preset-btn.prod        { border-color: rgba(239,68,68,0.3); }
.env-preset-btn.prod.active { background: rgba(239,68,68,0.2); color: #f87171; border-color: #f87171; }

.icon-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
}

.icon-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
