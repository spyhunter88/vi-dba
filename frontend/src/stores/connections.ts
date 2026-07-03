import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ConnectionConfig, DbObject, DbType } from '../types';
import { useUiStore } from './ui';
import { useTabStore } from './tabs';

export const useConnectionStore = defineStore('connections', () => {
  const uiStore = useUiStore();
  const connections = ref<ConnectionConfig[]>([]);
  const activeConnectionId = ref<string | null>(null);
  const activeDatabase = ref<string | null>(null);
  const activeSchema = ref<string | null>(null);
  const connectedIds = ref<Set<string>>(new Set());
  const expandedConnectionIds = ref<Set<string>>(new Set());
  const connectionObjects = ref<Record<string, DbObject[]>>({});

  const showConnectionModal = ref(false);
  const connectionModalType = ref<DbType | null>(null);
  const editingConnection = ref<ConnectionConfig | null>(null);

  const activeConnection = computed(() =>
    connections.value.find(c => c.id === activeConnectionId.value)
  );

  async function addConnection(config: ConnectionConfig) {
    try {
      await invoke('save_connection', { config });
      connections.value.push(config);
    } catch (e) {
      console.error('Failed to save connection:', e);
      throw e;
    }
  }

  async function updateConnection(config: ConnectionConfig) {
    try {
      await invoke('save_connection', { config });
      const index = connections.value.findIndex(c => c.id === config.id);
      if (index !== -1) {
        connections.value[index] = config;
      }
    } catch (e) {
      console.error('Failed to update connection:', e);
      throw e;
    }
  }

  async function loadConnections() {
    try {
      const saved = await invoke<ConnectionConfig[]>('get_connections');
      connections.value = saved;
    } catch (e) {
      console.error('Failed to load connections:', e);
    }
  }

  async function deleteConnection(id: string) {
    try {
      await disconnect(id);
      await invoke('delete_connection', { id });
      connections.value = connections.value.filter(c => c.id !== id);

      if (activeConnectionId.value === id) {
        activeConnectionId.value = null;
      }
      uiStore.showToast('Connection deleted and tabs detached.');
      // Note: Tab detachment will be handled by TabStore or via events/watchers
    } catch (e) {
      console.error('Failed to delete connection:', e);
    }
  }

  async function connect(id: string) {
    const config = connections.value.find(c => c.id === id);
    if (!config) return;

    try {
      const result = await invoke<{ serverVersion?: string }>('connect', { config });

      // Update server version if it changed
      if (result.serverVersion && result.serverVersion !== config.serverVersion) {
        config.serverVersion = result.serverVersion;
        // Save the updated configuration with the version
        await invoke('save_connection', { config });
      }

      connectedIds.value.add(id);
      connectedIds.value = new Set(connectedIds.value);
      expandedConnectionIds.value.add(id);
      expandedConnectionIds.value = new Set(expandedConnectionIds.value);

      // Only claim the sidebar selection if the user isn't already working inside
      // another connection's tab. Connecting can resolve late (e.g. the user expanded
      // a connection then switched to a different tab-group while it loaded); without
      // this guard that late connect would steal selection from the active tab.
      const activeTabConn = useTabStore().activeTab?.connectionId;
      if (!activeTabConn || activeTabConn === id) {
        activeConnectionId.value = id;
      }

      await refreshObjects(id);
    } catch (e) {
      console.error('Failed to connect:', e);
      throw e;
    }
  }

  async function disconnect(id: string) {
    try {
      await invoke('disconnect', { id });
      connectedIds.value.delete(id);
      connectedIds.value = new Set(connectedIds.value);
      expandedConnectionIds.value.delete(id);
      expandedConnectionIds.value = new Set(expandedConnectionIds.value);

      if (activeConnectionId.value === id) {
        activeConnectionId.value = null;
      }
    } catch (e) {
      console.error('Failed to disconnect:', e);
    }
  }

  async function refreshObjects(id: string) {
    try {
      const objects = await invoke<DbObject[]>('get_objects', { id });
      console.log('[refreshObjects] Objects:', objects);
      connectionObjects.value = { ...connectionObjects.value, [id]: objects };
    } catch (e) {
      console.error('Failed to get objects:', e);
    }
  }

  function setActiveContext(db: string | null, schema: string | null) {
    activeDatabase.value = db;
    activeSchema.value = schema;
  }

  function openNewConnectionModal(type: DbType | null = null) {
    editingConnection.value = null;
    connectionModalType.value = type;
    showConnectionModal.value = true;
  }

  function openEditConnectionModal(connection: ConnectionConfig) {
    editingConnection.value = { ...connection };
    connectionModalType.value = connection.dbType;
    showConnectionModal.value = true;
  }

  return {
    connections,
    activeConnectionId,
    activeDatabase,
    activeSchema,
    connectedIds,
    expandedConnectionIds,
    connectionObjects,
    showConnectionModal,
    connectionModalType,
    editingConnection,
    activeConnection,
    addConnection,
    updateConnection,
    loadConnections,
    deleteConnection,
    connect,
    disconnect,
    refreshObjects,
    setActiveContext,
    openNewConnectionModal,
    openEditConnectionModal
  };
});
