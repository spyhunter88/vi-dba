<script setup lang="ts">
import { Database, Play, History, Shield, Power } from 'lucide-vue-next';
import type { DbType } from '../types';
import { useConnectionStore } from '../stores/connections';
import { invoke } from '@tauri-apps/api/core';

const connectionStore = useConnectionStore();

const emit = defineEmits(['add-connection']);

function handleQuickConnect(type: DbType) {
  emit('add-connection', type);
}

function handleRecentClick(id: string) {
  if (connectionStore.connectedIds.has(id)) {
    connectionStore.disconnect(id);
  } else {
    connectionStore.connect(id);
  }
}
</script>

<template>
  <div class="welcome-screen flex-center flex-direction-column glass">
    <div class="welcome-header">
      <div class="logo-wrapper">
        <img src="/app-icon.png" alt="ViDBA" class="app-logo" />
      </div>
      <h1>ViDBA</h1>
      <p>The modern, lightweight database tools</p>
    </div>

    <div class="quick-actions">
      <h2>Quick Connect</h2>
      <div class="action-grid">
        <div class="action-card" @click="handleQuickConnect('mySQL')">
          <div class="icon-circle mysql">
            <Database :size="24" />
          </div>
          <span>MySQL</span>
        </div>
        <div class="action-card" @click="handleQuickConnect('postgreSQL')">
          <div class="icon-circle postgres">
            <Database :size="24" />
          </div>
          <span>PostgreSQL</span>
        </div>
        <div class="action-card" @click="handleQuickConnect('sqlServer')">
          <div class="icon-circle mssql">
            <Database :size="24" />
          </div>
          <span>SQL Server</span>
        </div>
        <div class="action-card" @click="handleQuickConnect('sqlite')">
          <div class="icon-circle sqlite">
            <Database :size="24" />
          </div>
          <span>SQLite</span>
        </div>
        <div class="action-card" @click="handleQuickConnect('oracle')">
          <div class="icon-circle oracle">
            <Database :size="24" />
          </div>
          <span>Oracle</span>
        </div>
        <div class="action-card" @click="handleQuickConnect('mongoDB')">
          <div class="icon-circle mongodb">
            <Database :size="24" />
          </div>
          <span>MongoDB</span>
        </div>
      </div>
    </div>

    <div v-if="connectionStore.connections.length > 0" class="recent-connections">
      <h2>Recent Connections</h2>
      <div class="recent-list">
        <div 
          v-for="conn in connectionStore.connections.slice(0, 5)" 
          :key="conn.id" 
          class="recent-item flex-between"
          :class="{ connected: connectionStore.connectedIds.has(conn.id) }"
          @click="handleRecentClick(conn.id)"
        >
          <div class="item-left flex-center gap-3">
             <div class="indicator-group flex-center flex-direction-column">
               <div class="type-indicator" :class="conn.dbType.toLowerCase()"></div>
               <div class="status-badge-mini" v-if="connectionStore.connectedIds.has(conn.id)"></div>
             </div>
             <div class="item-info">
                <div class="item-name">{{ conn.name }}</div>
                <div class="item-host">{{ conn.user }}@{{ conn.host }}</div>
             </div>
          </div>
          <div class="item-action">
            <Power v-if="connectionStore.connectedIds.has(conn.id)" :size="14" class="power-icon" />
            <Play v-else :size="14" class="play-icon" />
          </div>
        </div>
      </div>
    </div>

    <div class="welcome-footer">
      <div class="footer-item" @click="invoke('open_history')">
        <History :size="16" />
        <span>Open History</span>
      </div>
      <div class="footer-item" @click="invoke('open_security')">
        <Shield :size="16" />
        <span>Security Settings</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.welcome-screen {
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  padding: 40px;
  overflow-y: auto;
  background: radial-gradient(circle at 50% 50%, var(--accent-shadow) 0%, transparent 100%);
}

.welcome-header {
  text-align: center;
  margin-bottom: 48px;
}

.logo-wrapper {
  margin-bottom: 16px;
  animation: float 6s ease-in-out infinite;
}

@keyframes float {
  0% { transform: translateY(0px); }
  50% { transform: translateY(-10px); }
  100% { transform: translateY(0px); }
}

.text-accent {
  color: var(--accent-primary);
  filter: drop-shadow(0 0 15px var(--accent-shadow));
}

h1 {
  font-size: 1.75rem;
  font-weight: 800;
  margin-bottom: 8px;
  letter-spacing: -0.02em;
}

p {
  color: var(--text-secondary);
  font-size: 0.9rem;
}

h2 {
  font-size: 0.9rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-secondary);
  margin-bottom: 16px;
  text-align: center;
}

.quick-actions {
  width: 100%;
  max-width: 600px;
  margin-bottom: 48px;
}

.action-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
}

.action-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.action-card:hover {
  transform: translateY(-4px);
  background: var(--bg-tertiary);
  border-color: var(--accent-primary);
  box-shadow: var(--shadow-lg);
}

.icon-circle {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
}

.mysql { background: linear-gradient(135deg, #00758f, #f29111); }
.postgres { background: linear-gradient(135deg, #33a9dc, #12739e); }
.mssql { background: linear-gradient(135deg, #eb2d35, #a91d22); }
.sqlite { background: linear-gradient(135deg, #00b0ed, #003b57); }
.oracle { background: linear-gradient(135deg, #f80000, #c74634); }
.mongodb { background: linear-gradient(135deg, #4db33d, #3fa037); }

.recent-connections {
  width: 100%;
  max-width: 600px;
}

.recent-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.recent-item {
  background: var(--glass-bg);
  border: 1px solid var(--glass-border);
  border-radius: 8px;
  padding: 12px 16px;
  cursor: pointer;
  transition: all 0.2s;
}

.recent-item:hover {
  background: var(--glass-border);
  transform: translateX(4px);
}

.type-indicator {
  width: 4px;
  height: 24px;
  border-radius: 2px;
}

.type-indicator.mysql { background: #f29111; }
.type-indicator.postgresql { background: #33a9dc; }
.type-indicator.sqlserver { background: #eb2d35; }
.type-indicator.sqlite { background: #00b0ed; }
.type-indicator.oracle { background: #f80000; }
.type-indicator.mongodb { background: #4db33d; }

.indicator-group {
  gap: 4px;
  width: 8px;
}

.status-badge-mini {
  width: 6px;
  height: 6px;
  background: #10b981;
  border-radius: 50%;
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
}

.recent-item {
  position: relative;
}

.item-name {
  font-weight: 600;
  font-size: 0.9rem;
}

.item-host {
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.play-icon {
  opacity: 0;
  transition: opacity 0.2s;
}

.power-icon {
  opacity: 0.8;
  color: #ef4444;
  transition: opacity 0.2s;
}

.recent-item:hover .play-icon {
  opacity: 0.8;
}

.recent-item:hover .power-icon {
  opacity: 1;
}

.recent-item.connected {
  border-color: var(--accent-primary);
  background: rgba(59, 130, 246, 0.05);
}

.power-icon {
  color: #ef4444;
}

.welcome-footer {
  margin-top: 64px;
  display: flex;
  gap: 32px;
}

.footer-item {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 0.85rem;
  cursor: pointer;
}

.footer-item:hover {
  color: var(--text-primary);
}

.flex-direction-column {
  flex-direction: column;
}

.gap-3 {
  gap: 12px;
}

.app-logo {
  width: 48px;
  height: 48px;
  object-fit: contain;
  filter: drop-shadow(0 0 15px var(--accent-shadow));
}

.type-icon-img {
  width: 24px;
  height: 24px;
  object-fit: contain;
  filter: brightness(0) invert(1);
}
</style>
