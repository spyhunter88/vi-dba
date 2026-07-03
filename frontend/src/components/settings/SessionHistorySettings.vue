<script setup lang="ts">
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { useQueryStore } from '../../stores/query';
import { Trash2 } from 'lucide-vue-next';

const uiStore = useUiStore();
const sessionStore = useSessionStore();
const queryStore = useQueryStore();

async function handleClearHistory() {
  if (confirm('Are you sure you want to clear the query execution history? This cannot be undone.')) {
    await queryStore.clearQueryHistory();
    uiStore.showToast('Query history cleared', 'success');
  }
}
</script>

<template>
  <div class="settings-content">
    <h2>Session & History</h2>
    
    <div class="settings-section">
      <h3>Session Restoration</h3>
      <div class="settings-row">
        <div class="row-label">
          <span>Restore Session on Startup</span>
          <p>Automatically re-open tabs when the application starts.</p>
        </div>
        <div class="row-action">
          <label class="switch">
            <input type="checkbox" v-model="sessionStore.enableSessionRestore">
            <span class="slider round"></span>
          </label>
        </div>
      </div>
    </div>

    <div class="settings-section">
      <h3>Smart Snapshots <span class="badge">beta</span></h3>
      <div class="settings-row flex-column" :class="{ expanded: sessionStore.enableSnapshots }">
        <div class="row-header w-full">
          <div class="row-label">
            <span>Enable Intelligent Snapshots</span>
            <p>Create local backups of tabs when running queries or switching context.</p>
          </div>
          <div class="row-action">
            <label class="switch">
              <input type="checkbox" v-model="sessionStore.enableSnapshots">
              <span class="slider round"></span>
            </label>
          </div>
        </div>

        <div class="row-content w-full" v-if="sessionStore.enableSnapshots">
          <!-- Limit by Count -->
          <div class="sub-setting-row">
            <div class="flex-row gap-3">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="sessionStore.enableSnapshotLimitCount">
                <span class="checkmark"></span>
              </label>
              <span class="cursor-pointer" @click="sessionStore.enableSnapshotLimitCount = !sessionStore.enableSnapshotLimitCount">Limit by Count</span>
            </div>
            
            <input 
              type="number" 
              v-model="sessionStore.snapshotRetentionLimit" 
              min="5" 
              max="100" 
              class="settings-input"
              :disabled="!sessionStore.enableSnapshotLimitCount"
            >
          </div>

          <!-- Limit by Time -->
          <div class="sub-setting-row mt-3">
             <div class="flex-row gap-3">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="sessionStore.enableSnapshotLimitDays">
                <span class="checkmark"></span>
              </label>
              <span class="cursor-pointer" @click="sessionStore.enableSnapshotLimitDays = !sessionStore.enableSnapshotLimitDays">Limit by Time (Days)</span>
            </div>

            <input 
              type="number" 
              v-model="sessionStore.snapshotRetentionDays" 
              min="1" 
              max="365" 
              class="settings-input"
              :disabled="!sessionStore.enableSnapshotLimitDays"
            >
          </div>
        </div>
      </div>
    </div>

    <div class="settings-section">
      <h3>Query History Retention</h3>
      <div class="settings-row flex-column expanded">
        <div class="row-content w-full">
          <!-- Limit by Total Count -->
          <div class="sub-setting-row">
            <div class="flex-row gap-3">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="sessionStore.enableHistoryRetentionTotal">
                <span class="checkmark"></span>
              </label>
              <span class="cursor-pointer" @click="sessionStore.enableHistoryRetentionTotal = !sessionStore.enableHistoryRetentionTotal">Global Total Limit</span>
            </div>
            
            <input 
              type="number" 
              v-model="sessionStore.historyMaxTotal" 
              min="10" 
              class="settings-input"
              :disabled="!sessionStore.enableHistoryRetentionTotal"
            >
          </div>

          <!-- Limit per Connection -->
          <div class="sub-setting-row mt-3">
            <div class="flex-row gap-3">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="sessionStore.enableHistoryRetentionPerConnection">
                <span class="checkmark"></span>
              </label>
              <span class="cursor-pointer" @click="sessionStore.enableHistoryRetentionPerConnection = !sessionStore.enableHistoryRetentionPerConnection">Limit per Connection</span>
            </div>
            
            <input 
              type="number" 
              v-model="sessionStore.historyMaxPerConnection" 
              min="1" 
              class="settings-input"
              :disabled="!sessionStore.enableHistoryRetentionPerConnection"
            >
          </div>

          <!-- Limit by Lifetime -->
          <div class="sub-setting-row mt-3">
            <div class="flex-row gap-3">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="sessionStore.enableHistoryRetentionLifetime">
                <span class="checkmark"></span>
              </label>
              <span class="cursor-pointer" @click="sessionStore.enableHistoryRetentionLifetime = !sessionStore.enableHistoryRetentionLifetime">Retention Lifetime</span>
            </div>
            
            <div class="flex-row gap-2" v-if="sessionStore.enableHistoryRetentionLifetime">
              <div class="time-input-group">
                <input type="number" v-model="sessionStore.historyMaxLifetimeDays" min="0" class="settings-input sm">
                <span class="unit">d</span>
              </div>
              <div class="time-input-group">
                <input type="number" v-model="sessionStore.historyMaxLifetimeHours" min="0" max="23" class="settings-input sm">
                <span class="unit">h</span>
              </div>
              <div class="time-input-group">
                <input type="number" v-model="sessionStore.historyMaxLifetimeMinutes" min="0" max="59" class="settings-input sm">
                <span class="unit">m</span>
              </div>
            </div>
            <div v-else class="text-xs opacity-30">Disabled</div>
          </div>
        </div>
      </div>
    </div>

    <div class="settings-section">
      <h3>Data Management</h3>
      <div class="settings-row">
        <div class="row-label">
           <span>Clear History</span>
           <p>Remove all logs of executed queries.</p>
        </div>
        <div class="row-action">
           <button class="button-danger sm flex-center gap-2" @click="handleClearHistory">
             <Trash2 :size="14" />
             Clear Query History
           </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-content {
  padding: 24px;
}

h2 {
  font-size: 1.5rem;
  margin-bottom: 24px;
  color: var(--text-primary);
}

h3 {
  font-size: 1rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
  margin-bottom: 16px;
  margin-top: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 32px;
}

.settings-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  transition: all 0.3s ease;
}

.settings-row.expanded {
  flex-direction: column;
  align-items: stretch;
  gap: 16px;
}

.row-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.row-content {
  padding-left: 12px;
  padding-top: 4px;
}

.sub-setting-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.mt-3 {
  margin-top: 12px;
}

.row-label span {
  font-weight: 600;
  display: block;
  margin-bottom: 4px;
}

.row-label p {
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.settings-input {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 8px 12px;
  border-radius: 4px;
  width: 80px;
  text-align: center;
}

.settings-input.sm {
  width: 50px;
  padding: 6px 8px;
  font-size: 0.85rem;
}

.time-input-group {
  display: flex;
  align-items: center;
  gap: 4px;
}

.unit {
  font-size: 0.75rem;
  opacity: 0.4;
  text-transform: lowercase;
}

.badge {
  font-size: 0.7rem;
  background: var(--accent-primary);
  color: white;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
  font-weight: bold;
}

.cursor-pointer { cursor: pointer; }
.text-xs { font-size: 0.75rem; }
.opacity-30 { opacity: 0.3; }

/* Toggle Switch Styles */
.switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
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
  background-color: var(--bg-secondary);
  transition: .2s;
  border: 1px solid var(--border-color);
}

.slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 2px;
  bottom: 2px;
  background-color: var(--text-secondary);
  transition: .2s;
}

input:checked + .slider {
  background-color: var(--accent-primary);
  border-color: var(--accent-primary);
}

input:focus + .slider {
  box-shadow: 0 0 1px var(--accent-primary);
}

input:checked + .slider:before {
  transform: translateX(20px);
  background-color: white;
}

.slider.round {
  border-radius: 24px;
}

.slider.round:before {
  border-radius: 50%;
}

/* Custom Checkbox */
.custom-checkbox {
  display: block;
  position: relative;
  padding-left: 0;
  cursor: pointer;
  font-size: 22px;
  -webkit-user-select: none;
  -moz-user-select: none;
  -ms-user-select: none;
  user-select: none;
  display: flex;
  align-items: center;
  height: 20px;
  width: 20px;
}

.custom-checkbox input {
  position: absolute;
  opacity: 0;
  cursor: pointer;
  height: 0;
  width: 0;
}

.checkmark {
  position: absolute;
  top: 0;
  left: 0;
  height: 20px;
  width: 20px;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  transition: all 0.2s;
}

.custom-checkbox:hover input ~ .checkmark {
  border-color: var(--text-secondary);
}

.custom-checkbox input:checked ~ .checkmark {
  background-color: var(--accent-primary);
  border-color: var(--accent-primary);
}

.checkmark:after {
  content: "";
  position: absolute;
  display: none;
}

.custom-checkbox input:checked ~ .checkmark:after {
  display: block;
}

.custom-checkbox .checkmark:after {
  left: 7px;
  top: 3px;
  width: 5px;
  height: 10px;
  border: solid white;
  border-width: 0 2px 2px 0;
  -webkit-transform: rotate(45deg);
  -ms-transform: rotate(45deg);
  transform: rotate(45deg);
}

.flex-row {
  display: flex;
  align-items: center;
}

.gap-3 {
  gap: 12px;
}

</style>
