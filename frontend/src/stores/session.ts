import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { SessionState } from '../types';
import { useConnectionStore } from './connections';
import { useTabStore } from './tabs';
import { useQueryStore } from './query';
import { useUiStore } from './ui';

export const useSessionStore = defineStore('session', () => {
  const connectionStore = useConnectionStore();
  const tabStore = useTabStore();
  const queryStore = useQueryStore();
  const uiStore = useUiStore();

  const enableSessionRestore = ref(true);
  const enableSnapshots = ref(false);
  const enableSnapshotLimitCount = ref(true);
  const snapshotRetentionLimit = ref(20);
  const enableSnapshotLimitDays = ref(false);
  const snapshotRetentionDays = ref(7);
  const aiMode = ref<string>('integrated');

  // Query History Retention
  const enableHistoryRetentionTotal = ref(true);
  const historyMaxTotal = ref(1000);
  const enableHistoryRetentionPerConnection = ref(false);
  const historyMaxPerConnection = ref(100);
  const enableHistoryRetentionLifetime = ref(false);
  const historyMaxLifetimeDays = ref(30);
  const historyMaxLifetimeHours = ref(0);
  const historyMaxLifetimeMinutes = ref(0);

  let saveSessionTimeout: any = null;

  function debouncedSaveSession() {
    if (saveSessionTimeout) clearTimeout(saveSessionTimeout);
    saveSessionTimeout = setTimeout(() => {
      saveSession();
    }, 2000);
  }

  async function saveSession() {
    const sessionState: SessionState = {
      tabs: tabStore.tabs.filter(t => ['sql_query', 'ai_sql', 'routine_editor', 'view_editor', 'table_data'].includes(t.type)).map(t => ({
        id: t.id,
        title: t.title,
        tabType: t.type,
        connectionId: t.connectionId,
        content: t.content,
        filePath: t.filePath,
        metadata: t.metadata,
        pagination: t.pagination,
      })),
      activeTabId: tabStore.activeTabId || undefined
    };

    try {
      await invoke('save_session', { state: sessionState });
    } catch (e) {
      console.error('Failed to save session:', e);
    }
  }

  async function restoreSession() {
    if (!enableSessionRestore.value) return;

    try {
      const state = await invoke<SessionState | null>('load_session');
      if (state && state.tabs && state.tabs.length > 0) {
        tabStore.tabs = state.tabs.map(t => {
          const connectionExists = connectionStore.connections.some(c => c.id === t.connectionId);
          return {
            id: t.id,
            title: t.title,
            type: t.tabType as any,
            connectionId: t.connectionId,
            content: t.content,
            filePath: t.filePath,
            metadata: t.metadata,
            pagination: t.pagination || ((t.tabType === 'table_data') ? { page: 1, pageSize: 1000 } : undefined),
            isDirty: false,
            isDetached: !connectionExists
          };
        });
        tabStore.activeTabId = state.activeTabId || (tabStore.tabs[tabStore.tabs.length - 1]?.id || null);

        tabStore.tabs.forEach(tab => {
          if (!tab.isDetached && tab.type === 'table_data') {
            if (connectionStore.connectedIds.has(tab.connectionId)) {
              queryStore.executeQueryInTab(tab.id);
            }
          }
        });

        const detachedCount = tabStore.tabs.filter(t => t.isDetached).length;
        if (detachedCount > 0) {
          uiStore.showToast(`Restored session. ${detachedCount} tabs are in detached mode.`, 'warning');
        } else {
          uiStore.showToast('Restored unsaved scripts from previous session.', 'info');
        }
      }
    } catch (e) {
      console.error('Failed to restore session:', e);
    }
  }

  async function saveSnapshot() {
    if (!enableSnapshots.value) return;
    const activeTab = tabStore.activeTab;
    if (!activeTab || !activeTab.content) return;

    try {
      const countLimit = enableSnapshotLimitCount.value ? snapshotRetentionLimit.value : null;
      const daysLimit = enableSnapshotLimitDays.value ? snapshotRetentionDays.value : null;

      await invoke('save_snapshot', {
        tabId: activeTab.id,
        connectionId: activeTab.connectionId,
        database: activeTab.database || null,
        schema: activeTab.schema || null,
        content: activeTab.content,
        limit: countLimit,
        limitDays: daysLimit
      });
      uiStore.showToast('Snapshot saved', 'success');
    } catch (e) {
      console.error('Failed to save snapshot:', e);
    }
  }

  async function getSnapshots(tabId: string, connectionId?: string, database?: string, schema?: string) {
    try {
      // If metadata not provided, try to find the tab to get it
      let connId = connectionId;
      let db = database;
      let sch = schema;

      if (!connId) {
        const tab = tabStore.tabs.find(t => t.id === tabId);
        if (tab) {
          connId = tab.connectionId;
          db = tab.database;
          sch = tab.schema;
        }
      }

      return await invoke<any[]>('get_snapshots', {
        tabId,
        connectionId: connId || '',
        database: db || null,
        schema: sch || null
      });
    } catch (e) {
      console.error('Failed to get snapshots:', e);
      return [];
    }
  }

  async function getAllSnapshotsSummary() {
    try {
      return await invoke<any[]>('get_all_snapshots_summary');
    } catch (e) {
      console.error('Failed to get all snapshots summary:', e);
      return [];
    }
  }

  function loadSettings() {
    const savedRestore = localStorage.getItem('enableSessionRestore');
    if (savedRestore !== null) enableSessionRestore.value = savedRestore === 'true';

    const savedSnapshots = localStorage.getItem('enableSnapshots');
    if (savedSnapshots !== null) enableSnapshots.value = savedSnapshots === 'true';

    const savedLimit = localStorage.getItem('snapshotRetentionLimit');
    if (savedLimit !== null) snapshotRetentionLimit.value = parseInt(savedLimit, 10);

    const savedLimitCountEnabled = localStorage.getItem('enableSnapshotLimitCount');
    if (savedLimitCountEnabled !== null) enableSnapshotLimitCount.value = savedLimitCountEnabled === 'true';

    const savedLimitDaysEnabled = localStorage.getItem('enableSnapshotLimitDays');
    if (savedLimitDaysEnabled !== null) enableSnapshotLimitDays.value = savedLimitDaysEnabled === 'true';

    const savedDays = localStorage.getItem('snapshotRetentionDays');
    if (savedDays !== null) snapshotRetentionDays.value = parseInt(savedDays, 10);
  }

  async function fetchAppSettings() {
    try {
      const s: any = await invoke('get_app_settings');
      if (s && s.aiMode) {
        aiMode.value = s.aiMode;
      }
      if (s) {
        enableHistoryRetentionTotal.value = s.enableHistoryRetentionTotal ?? true;
        historyMaxTotal.value = s.historyMaxTotal ?? 1000;
        enableHistoryRetentionPerConnection.value = s.enableHistoryRetentionPerConnection ?? false;
        historyMaxPerConnection.value = s.historyMaxPerConnection ?? 100;
        enableHistoryRetentionLifetime.value = s.enableHistoryRetentionLifetime ?? false;
        historyMaxLifetimeDays.value = s.historyMaxLifetimeDays ?? 30;
        historyMaxLifetimeHours.value = s.historyMaxLifetimeHours ?? 0;
        historyMaxLifetimeMinutes.value = s.historyMaxLifetimeMinutes ?? 0;
      }
    } catch (e) {
      console.error('Failed to fetch app settings:', e);
    }
  }

  loadSettings();

  // Auto-save session on tab changes
  watch([() => tabStore.tabs, () => tabStore.activeTabId], () => {
    debouncedSaveSession();
  }, { deep: true });

  // Listen to settings changes and save to localStorage
  watch([enableSessionRestore, enableSnapshots, snapshotRetentionLimit, enableSnapshotLimitCount, enableSnapshotLimitDays, snapshotRetentionDays], () => {
    localStorage.setItem('enableSessionRestore', String(enableSessionRestore.value));
    localStorage.setItem('enableSnapshots', String(enableSnapshots.value));
    localStorage.setItem('snapshotRetentionLimit', String(snapshotRetentionLimit.value));
    localStorage.setItem('enableSnapshotLimitCount', String(enableSnapshotLimitCount.value));
    localStorage.setItem('enableSnapshotLimitDays', String(enableSnapshotLimitDays.value));
    localStorage.setItem('snapshotRetentionDays', String(snapshotRetentionDays.value));
  });

  // Persist history settings to backend when changed
  watch([
    enableHistoryRetentionTotal, historyMaxTotal,
    enableHistoryRetentionPerConnection, historyMaxPerConnection,
    enableHistoryRetentionLifetime, historyMaxLifetimeDays, historyMaxLifetimeHours, historyMaxLifetimeMinutes
  ], async () => {
    try {
      // Get current settings first to preserve other fields
      const current: any = await invoke('get_app_settings');
      const updated = {
        ...current,
        enableHistoryRetentionTotal: enableHistoryRetentionTotal.value,
        historyMaxTotal: historyMaxTotal.value,
        enableHistoryRetentionPerConnection: enableHistoryRetentionPerConnection.value,
        historyMaxPerConnection: historyMaxPerConnection.value,
        enableHistoryRetentionLifetime: enableHistoryRetentionLifetime.value,
        historyMaxLifetimeDays: historyMaxLifetimeDays.value,
        historyMaxLifetimeHours: historyMaxLifetimeHours.value,
        historyMaxLifetimeMinutes: historyMaxLifetimeMinutes.value
      };
      await invoke('update_app_settings', { settings: updated });
    } catch (e) {
      console.error('Failed to update app settings:', e);
    }
  });

  return {
    enableSessionRestore,
    enableSnapshots,
    enableSnapshotLimitCount,
    snapshotRetentionLimit,
    enableSnapshotLimitDays,
    snapshotRetentionDays,
    aiMode,
    enableHistoryRetentionTotal,
    historyMaxTotal,
    enableHistoryRetentionPerConnection,
    historyMaxPerConnection,
    enableHistoryRetentionLifetime,
    historyMaxLifetimeDays,
    historyMaxLifetimeHours,
    historyMaxLifetimeMinutes,
    saveSession,
    restoreSession,
    debouncedSaveSession,
    saveSnapshot,
    getSnapshots,
    getAllSnapshotsSummary,
    loadSettings,
    fetchAppSettings
  };
});
