<template>
  <div class="settings-content">
    <h2>General</h2>
    <div class="settings-section">
      <div class="settings-row">
        <div class="row-label">
          <span>Language (Coming Soon)</span>
          <p>Select your preferred language interface.</p>
        </div>
        <div class="row-action">
          <select disabled class="settings-select">
            <option selected>English</option>
          </select>
        </div>
      </div>

      <div class="settings-row">
        <div class="row-label">
          <span>App Data Path</span>
          <p>Location for configurations, history, and scripts. Default is the current app path.</p>
        </div>
        <div class="row-action flex-col gap-2">
          <div class="flex gap-2">
            <input 
              type="text" 
              v-model="settings.appDataPath" 
              class="settings-input flex-1"
              placeholder="Default (Current App Path)"
            >
            <button class="btn btn-secondary btn-sm" @click="browsePath">Browse</button>
          </div>
          <button 
            class="btn btn-primary btn-sm self-end" 
            @click="saveSettings" 
            :disabled="saving"
          >
            {{ saving ? 'Saving...' : 'Save Path' }}
          </button>
        </div>
      </div>

      <div class="settings-row">
        <div class="row-label">
          <span>Schema Cache TTL (minutes)</span>
          <p>How long to keep database schema metadata in the local cache. 0 to disable cache.</p>
        </div>
        <div class="row-action">
          <input 
            type="number" 
            v-model.number="settings.schemaCacheTtl" 
            class="settings-input"
            placeholder="60"
            min="0"
          >
        </div>
      </div>

      <div class="settings-row">
        <div class="row-label">
          <span>Clear All Schema Cache</span>
          <p>Remove all cached database schemas from the local database.</p>
        </div>
        <div class="row-action">
          <button class="btn btn-secondary btn-sm text-error" @click="clearAllCache">Clear All Cache</button>
        </div>
      </div>

      <div class="settings-row">
        <div class="row-label">
          <span>Auto-connect on Startup</span>
          <p>Automatically restore previous active connections.</p>
        </div>
        <div class="row-action">
          <label class="switch">
            <input type="checkbox" disabled>
            <span class="slider round"></span>
          </label>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const settings = ref({
  appDataPath: '',
  schemaCacheTtl: 60
});
const saving = ref(false);

onMounted(async () => {
  try {
    const res = await invoke<any>('get_app_settings');
    settings.value.appDataPath = res.appDataPath || '';
    settings.value.schemaCacheTtl = res.schemaCacheTtl ?? 60;
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
});

const browsePath = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select App Data Directory'
    });
    
    if (selected && typeof selected === 'string') {
      settings.value.appDataPath = selected;
    }
  } catch (error) {
    console.error('Failed to browse path:', error);
  }
};

const saveSettings = async () => {
  saving.value = true;
  try {
    await invoke('update_app_settings', { 
      settings: { 
        appDataPath: settings.value.appDataPath || null,
        schemaCacheTtl: settings.value.schemaCacheTtl || 0
      } 
    });
    alert('Settings saved. Some changes may require app restart.');
  } catch (error) {
    console.error('Failed to save settings:', error);
    alert('Failed to save settings: ' + error);
  } finally {
    saving.value = false;
  }
};
const clearAllCache = async () => {
  if (confirm('Are you sure you want to clear ALL cached schemas?')) {
    try {
      await invoke('clear_all_schema_cache');
      alert('All schema cache cleared.');
    } catch (error) {
      console.error('Failed to clear cache:', error);
      alert('Failed to clear cache: ' + error);
    }
  }
};
</script>

<style scoped>
.settings-content {
  padding: 24px;
}

h2 {
  font-size: 1.25rem;
  margin-bottom: 24px;
  color: var(--text-primary);
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.settings-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.row-label span {
  font-weight: 600;
  display: block;
  margin-bottom: 4px;
}

.row-label p {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.settings-select {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 6px 12px;
  border-radius: 4px;
  min-width: 120px;
}

/* Toggle Switch Styles */
.switch {
  position: relative;
  display: inline-block;
  width: 40px;
  height: 20px;
  opacity: 0.5;
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--bg-tertiary);
  transition: .4s;
}

.slider:before {
  position: absolute;
  content: "";
  height: 14px;
  width: 14px;
  left: 3px;
  bottom: 3px;
  background-color: var(--text-secondary);
  transition: .4s;
}

.slider.round {
  border-radius: 20px;
}

.slider.round:before {
  border-radius: 50%;
}

.flex { display: flex; }
.flex-col { flex-direction: column; }
.flex-1 { flex: 1; }
.gap-2 { gap: 8px; }
.self-end { align-self: flex-end; }

.settings-input {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 8px 12px;
  border-radius: 4px;
  font-size: 0.9rem;
}

.btn {
  padding: 8px 16px;
  border-radius: 4px;
  border: none;
  cursor: pointer;
  font-weight: 500;
  transition: all 0.2s;
}

.btn-sm {
  padding: 4px 12px;
  font-size: 0.8rem;
}

.btn-primary {
  background: var(--accent-primary);
  color: white;
}

.btn-primary:hover {
  background: var(--accent-secondary);
}

.btn-secondary {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.btn-secondary:hover {
  background: var(--glass-border);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.text-error {
  color: #f87171 !important;
}

.text-error:hover {
  background: rgba(248, 113, 113, 0.1) !important;
}
</style>
