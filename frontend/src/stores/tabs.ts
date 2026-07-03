import { defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';
import type { Tab } from '../types';
import { useConnectionStore } from './connections';

export const useTabStore = defineStore('tabs', () => {
  const tabs = ref<Tab[]>([]);
  const activeTabId = ref<string | null>(null);
  const collapsedTabGroups = ref<Set<string>>(new Set());
  const showReconnectModal = ref(false);
  const tabToReconnect = ref<Tab | null>(null);

  const activeTab = computed(() =>
    tabs.value.find(t => t.id === activeTabId.value)
  );

  // The active tab is the single source of truth for the "selected" connection.
  // Point the sidebar's selected connection (and DB/schema context) at the given
  // tab, so new scripts/tables created afterwards belong to the connection you're
  // actually looking at — not whichever connection happened to finish connecting,
  // or was last clicked in the sidebar.
  function syncSidebarToTab(tab: Tab | undefined) {
    if (!tab || !tab.connectionId) return;
    const connStore = useConnectionStore();
    connStore.activeConnectionId = tab.connectionId;
    connStore.setActiveContext(tab.database ?? null, tab.schema ?? null);
  }

  // Fires for programmatic activations (addTab, closeTab, session restore) and for
  // user tab switches. Same-tab interactions are handled by activateTab() below,
  // since the watcher only runs when activeTabId actually changes.
  watch(activeTabId, (id) => {
    if (id) syncSidebarToTab(tabs.value.find(t => t.id === id));
  });

  // Make a tab active AND re-point the sidebar at it. Runs the sync even when the
  // tab is already active, so that after the user selects a different connection in
  // the sidebar, clicking back into a tab restores the highlight to that tab's own
  // connection.
  function activateTab(id: string) {
    activeTabId.value = id;
    syncSidebarToTab(tabs.value.find(t => t.id === id));
  }

  async function addTab(tab: Tab) {
    const existing = tabs.value.find(t => t.id === tab.id);
    if (existing) {
      activeTabId.value = existing.id;
      return;
    }

    if (tab.isDirty === undefined) {
      tab.isDirty = false;
    }

    if (tab.type === 'table_data' && !tab.pagination) {
      tab.pagination = { page: 1, pageSize: 1000 };
    }

    tabs.value.push(tab);
    activeTabId.value = tab.id;

    // Side effects (triggering initial load) will be handled by the component or a watcher 
    // to avoid circular dependencies with QueryStore for now.
    // Or we can emit an event.
  }

  function closeTab(id: string) {
    const index = tabs.value.findIndex(t => t.id === id);
    if (index === -1) return;

    const tab = tabs.value[index];
    if (tab && tab.isDirty) {
      if (!confirm(`You have unsaved changes in "${tab.title}". Are you sure you want to close it?`)) {
        return;
      }
    }

    tabs.value.splice(index, 1);
    if (activeTabId.value === id) {
      activeTabId.value = tabs.value.length > 0 ? (tabs.value[tabs.value.length - 1]?.id || null) : null;
    }
  }

  function reorderTabs(draggedTabId: string, targetTabId: string) {
    const fromIndex = tabs.value.findIndex(t => t.id === draggedTabId);
    const toIndex = tabs.value.findIndex(t => t.id === targetTabId);

    if (fromIndex !== -1 && toIndex !== -1) {
      const draggedTab = tabs.value[fromIndex];
      if (draggedTab) {
        tabs.value.splice(fromIndex, 1);
        tabs.value.splice(toIndex, 0, draggedTab);
      }
    }
  }

  function toggleTabGroupCollapse(connectionId: string) {
    if (collapsedTabGroups.value.has(connectionId)) {
      collapsedTabGroups.value.delete(connectionId);
    } else {
      collapsedTabGroups.value.add(connectionId);
    }
    collapsedTabGroups.value = new Set(collapsedTabGroups.value);
  }

  // Helper functions for specific tab types
  async function createTable(connectionId: string, catalog?: string, schema?: string) {
    const connStore = useConnectionStore();
    const db = catalog || connStore.activeDatabase || undefined;
    const sch = schema || connStore.activeSchema || undefined;

    addTab({
      id: `create-table-${connectionId}-${Date.now()}`,
      title: 'New Table',
      type: 'table_structure',
      connectionId,
      database: db,
      schema: sch,
      metadata: { catalog: db, schema: sch },
      isDirty: false
    });
  }

  async function createScript(connectionId: string, database?: string, schema?: string) {
    const connStore = useConnectionStore();
    const db = database || connStore.activeDatabase || undefined;
    const sch = schema || connStore.activeSchema || undefined;

    addTab({
      id: `query-${connectionId}-${Date.now()}`,
      title: 'New Script',
      type: 'sql_query',
      connectionId,
      database: db,
      schema: sch,
      content: '',
      isDirty: false
    });
  }

  async function editTable(connectionId: string, tableName: string, catalog?: string, schema?: string) {
    addTab({
      id: `edit-table-${connectionId}-${tableName}`,
      title: `Edit: ${tableName}`,
      type: 'table_structure',
      connectionId,
      database: catalog,
      schema,
      metadata: { tableName, catalog, schema },
      isDirty: false
    });
  }

  async function openScriptList(connectionId: string) {
    addTab({
      id: `scripts-${connectionId}`,
      title: 'Scripts',
      type: 'script_list',
      connectionId,
      isDirty: false
    });
  }

  async function editRoutine(connectionId: string, name: string, routineType: string, catalog?: string, schema?: string) {
    addTab({
      id: `routine-${connectionId}-${name}`,
      title: `${name}`,
      type: 'routine_editor',
      connectionId,
      database: catalog,
      schema,
      metadata: { name, routineType, catalog, schema },
      isDirty: false
    });
  }

  async function openRoutineList(connectionId: string, catalog?: string, schema?: string) {
    addTab({
      id: `routines-${connectionId}-${catalog || ''}-${schema || ''}`,
      title: 'Procedures & Functions',
      type: 'routine_list',
      connectionId,
      database: catalog,
      schema,
      metadata: { catalog, schema },
      isDirty: false
    });
  }

  async function editView(connectionId: string, name: string, catalog?: string, schema?: string) {
    addTab({
      id: `view-${connectionId}-${name}`,
      title: `View: ${name}`,
      type: 'view_editor',
      connectionId,
      database: catalog,
      schema,
      metadata: { name, catalog, schema },
      isDirty: false
    });
  }

  async function openAiSqlEditor(connectionId: string, database?: string, schema?: string) {
    const connStore = useConnectionStore();
    const db = database || connStore.activeDatabase || undefined;
    const sch = schema || connStore.activeSchema || undefined;

    addTab({
      id: `ai-sql-${connectionId}-${Date.now()}`,
      title: 'AI SQL Editor',
      type: 'ai_sql',
      connectionId,
      database: db,
      schema: sch,
      content: '',
      isDirty: false
    });
  }

  async function openAiRoutineEditor(connectionId: string, database?: string, schema?: string) {
    const connStore = useConnectionStore();
    const db = database || connStore.activeDatabase || undefined;
    const sch = schema || connStore.activeSchema || undefined;

    addTab({
      id: `ai-routine-${connectionId}-${Date.now()}`,
      title: 'AI Routine Editor',
      type: 'ai_routine_editor',
      connectionId,
      database: db,
      schema: sch,
      content: '',
      isDirty: false
    });
  }

  function toggleAiPrompt(id: string) {
    const tab = tabs.value.find(t => t.id === id);
    if (tab) {
      tab.aiPromptOpen = !tab.aiPromptOpen;
    }
  }

  function openReconnectModal(tab: Tab) {
    tabToReconnect.value = tab;
    showReconnectModal.value = true;
  }

  function closeReconnectModal() {
    showReconnectModal.value = false;
    tabToReconnect.value = null;
  }

  return {
    tabs,
    activeTabId,
    collapsedTabGroups,
    activeTab,
    addTab,
    activateTab,
    closeTab,
    reorderTabs,
    toggleTabGroupCollapse,
    createTable,
    createScript,
    editTable,
    openScriptList,
    editRoutine,
    openRoutineList,
    editView,
    openAiSqlEditor,
    openAiRoutineEditor,
    toggleAiPrompt,
    showReconnectModal,
    tabToReconnect,
    openReconnectModal,
    closeReconnectModal
  };
});
