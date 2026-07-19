<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useConnectionStore } from '../../stores/connections';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useUiStore } from '../../stores/ui';
import {
  HardDrive,
  Database,
  Plus,
  ChevronRight,
  ChevronDown,
  Play,
  Power,
  RefreshCw,
  Columns,
  ChevronsLeft,
  ChevronsRight,
  MoreVertical,
  Edit2,
  Trash2,
  Settings,
  Download,
  Upload,
  Sparkles,
  Search,
  FileText,
  Clock,
  History,
  List,
  Layers,
  GitBranch,
} from 'lucide-vue-next';
import type { ConnectionConfig } from '../../types';
import { invoke } from '@tauri-apps/api/core';

import SidebarTreeNode from './SidebarTreeNode.vue';
import type { DbObject, DbType } from '../../types';

const connectionStore = useConnectionStore();
const tabStore = useTabStore();
const queryStore = useQueryStore();
const uiStore = useUiStore();
const router = useRouter();
const connectionSearchQuery = ref('');
const connectionNodeSearchQueries = ref<Record<string, string>>({});

const filteredConnections = computed(() => {
  if (!connectionSearchQuery.value) return connectionStore.connections;
  const q = connectionSearchQuery.value.toLowerCase();
  return connectionStore.connections.filter(c => c.name.toLowerCase().includes(q));
});

// ── View mode ──────────────────────────────────────────────────────────────
type ViewMode = 'flat' | 'product' | 'environment' | 'dbtype';
interface ConnGroup { key: string; label: string; connections: ConnectionConfig[]; }

const viewMode = ref<ViewMode>((localStorage.getItem('sidebar-view-mode') as ViewMode) || 'flat');
const collapsedGroups = ref<Set<string>>(new Set());

watch(viewMode, v => localStorage.setItem('sidebar-view-mode', v));

function toggleGroup(key: string) {
  const s = new Set(collapsedGroups.value);
  s.has(key) ? s.delete(key) : s.add(key);
  collapsedGroups.value = s;
}

const DB_TYPE_LABELS: Record<string, string> = {
  mySQL: 'MySQL', postgreSQL: 'PostgreSQL', sqlServer: 'SQL Server',
  sqlite: 'SQLite', oracle: 'Oracle', mongoDB: 'MongoDB',
};
const ENV_ORDER = ['dev', 'test', 'staging', 'qa', 'uat', 'beta', 'prod', 'production'];

const connectionGroups = computed<ConnGroup[]>(() => {
  const conns = filteredConnections.value;
  if (viewMode.value === 'flat') return [{ key: '_all', label: '', connections: conns }];

  const map: Record<string, ConnGroup> = {};
  for (const conn of conns) {
    let key: string;
    let label: string;
    if (viewMode.value === 'product') {
      key = conn.group || '_ungrouped';
      label = conn.group || 'Ungrouped';
    } else if (viewMode.value === 'environment') {
      key = conn.environment || '_ungrouped';
      label = conn.environment
        ? conn.environment.charAt(0).toUpperCase() + conn.environment.slice(1)
        : 'Ungrouped';
    } else {
      key = conn.dbType;
      label = DB_TYPE_LABELS[conn.dbType] || conn.dbType;
    }
    if (!map[key]) map[key] = { key, label, connections: [] };
    map[key]?.connections.push(conn);
  }

  return Object.values(map).sort((a, b) => {
    if (a.key === '_ungrouped') return 1;
    if (b.key === '_ungrouped') return -1;
    if (viewMode.value === 'environment') {
      const ai = ENV_ORDER.indexOf(a.key), bi = ENV_ORDER.indexOf(b.key);
      if (ai !== -1 && bi !== -1) return ai - bi;
      if (ai !== -1) return -1;
      if (bi !== -1) return 1;
    }
    return a.label.localeCompare(b.label);
  });
});

async function toggleConnection(id: string) {
  // Clicking a connection makes it the app's current/selected connection (the
  // sidebar highlight moves to it) while leaving the open workspace tab untouched.
  // This is an explicit, synchronous selection — unlike connect()'s, it isn't
  // guarded against the active tab, so it always wins at click time. Interacting
  // with a tab afterwards re-syncs the highlight back to that tab's connection.
  if (connectionStore.activeConnectionId !== id) {
    connectionStore.setActiveContext(null, null);
  }
  connectionStore.activeConnectionId = id;

  if (connectionStore.expandedConnectionIds.has(id)) {
    connectionStore.expandedConnectionIds.delete(id);
  } else {
    connectionStore.expandedConnectionIds.add(id);
    if (!connectionStore.connectedIds.has(id)) {
      try {
        await connectionStore.connect(id);
      } catch (e) {
        connectionStore.expandedConnectionIds.delete(id);
        uiStore.showToast('Failed to connect: ' + e, 'error');
      }
    }
  }
}

function handleObjectClick(connId: string, object: any) {
  if (object.objectType === 'table' || object.objectType === 'view') {
    tabStore.addTab({
      id: `table-${connId}-${object.name}`,
      title: object.name,
      type: 'table_data',
      connectionId: connId,
      database: object.catalog,
      schema: object.schema,
      metadata: { tableName: object.name, catalog: object.catalog, schema: object.schema }
    });
  } else if (object.objectType === 'procedure' || object.objectType === 'function') {
    tabStore.editRoutine(connId, object.name, object.objectType, object.catalog, object.schema);
  }
  
  // Set active context
  connectionStore.setActiveContext(object.catalog || null, object.schema || null);
}

const getConnIcon = (type: DbType) => {
  if (type === 'sqlite') return HardDrive;
  return Database;
};

function handleOpenList(connId: string, node: any) {
  const catalog = node.catalog;
  const schema = node.schema;
  const title = node.name === 'Tables' || node.name === 'Procedures' || node.name === 'Functions' 
    ? (schema || catalog || node.name) 
    : node.name;
  
  if (node.name === 'Procedures' || node.name === 'Functions') {
    tabStore.openRoutineList(connId, catalog, schema);
    return;
  }

  tabStore.addTab({
    id: `table-list-${connId}-${catalog}-${schema}`,
    title: `List: ${title}`,
    type: 'table_list',
    connectionId: connId,
    database: catalog,
    schema,
    metadata: { catalog, schema }
  });

  // Set active context
  connectionStore.setActiveContext(catalog || null, schema || null);
}

function handleOpenSettings() {
  invoke('open_settings');
}

function buildTree(objects: DbObject[]) {
  const tree: any[] = [];
  const catalogs: Record<string, any> = {};

  objects.forEach(obj => {
    // Skip objects with missing/empty type — prevents broken category names like "S"
    if (!obj.objectType) return;

    const catalogName = obj.catalog || 'Default';
    if (!catalogs[catalogName]) {
      catalogs[catalogName] = {
        id: `catalog-${catalogName}`,
        name: catalogName,
        type: 'catalog',
        catalog: obj.catalog || null, // Preserve actual null if it was null
        children: {},
        _categories: {}
      };
      tree.push(catalogs[catalogName]);
    }

    const catalog = catalogs[catalogName];
    let parentNode = catalog;

    if (obj.schema) {
      if (!catalog.children[obj.schema]) {
        catalog.children[obj.schema] = {
          id: `schema-${catalogName}-${obj.schema}`,
          name: obj.schema,
          type: 'schema',
          catalog: obj.catalog || null, // metadata
          schema: obj.schema, // metadata
          children: {},
          _categories: {}
        };
      }
      parentNode = catalog.children[obj.schema];
    }

    // Category Level — capitalize the pluralized type: "table" → "Tables"
    const rawCategory = obj.objectType + 's';
    const categoryName = rawCategory.charAt(0).toUpperCase() + rawCategory.slice(1);
    if (!parentNode._categories[categoryName]) {
      parentNode._categories[categoryName] = {
        id: `cat-${parentNode.id}-${categoryName}`,
        name: categoryName,
        type: 'category',
        catalog: parentNode.catalog, // metadata
        schema: parentNode.schema, // metadata
        children: []
      };
    }
    
    parentNode._categories[categoryName].children.push({
      id: `obj-${parentNode.id}-${obj.name}`,
      name: obj.name,
      type: obj.objectType,
      catalog: parentNode.catalog, // Preserve actual null if parent.catalog was null
      schema: parentNode.schema // metadata
    });
  });

  // Convert children and categories maps to arrays
  const finalize = (nodes: any[]) => {
    nodes.forEach(node => {
      if (node.children && !Array.isArray(node.children)) {
        node.children = Object.values(node.children);
        // Sort schemas/catalogs
        node.children.sort((a: any, b: any) => a.name.localeCompare(b.name));
      }
      if (node._categories) {
        const categories = Object.values(node._categories);
        // Sort categories with a fixed priority: Tables first, then Views, Procedures, Functions, rest alphabetically
        const categoryOrder: Record<string, number> = { Tables: 0, Views: 1, Procedures: 2, Functions: 3 };
        categories.sort((a: any, b: any) => {
          const ao = categoryOrder[a.name] ?? 99;
          const bo = categoryOrder[b.name] ?? 99;
          return ao !== bo ? ao - bo : a.name.localeCompare(b.name);
        });
        node.children = [...(node.children || []), ...categories];
        delete node._categories;
      }
      if (node.children) finalize(node.children);
    });
  };

  finalize(tree);
  
  // Sort everything at the root level (catalogs/schemas)
  tree.sort((a, b) => a.name.localeCompare(b.name));
  
  return tree;
}

const connectionTrees = computed(() => {
  const trees: Record<string, any[]> = {};
  for (const connId in connectionStore.connectionObjects) {
    let objects = [...(connectionStore.connectionObjects[connId] || [])];
    
    // Sort objects by name initially
    objects.sort((a, b) => a.name.localeCompare(b.name));
    
    // Filter objects by name if a search query exists for this connection
    const query = connectionNodeSearchQueries.value[connId]?.toLowerCase();
    if (query) {
      objects = objects.filter(obj => obj.name.toLowerCase().includes(query));
    }
    
    trees[connId] = buildTree(objects);
  }
  return trees;
});

function handleNewConnection(type?: DbType) {
  connectionStore.openNewConnectionModal(type);
  showAddDropdown.value = false;
}

const showAddDropdown = ref(false);
const showFooterAddDropdown = ref(false);

const menuVisible = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const menuConnectionId = ref<string | null>(null);
const connMenuEl = ref<HTMLElement | null>(null);

function showContextMenu(e: MouseEvent, connId: string) {
  e.preventDefault();
  e.stopPropagation();
  menuVisible.value = true;
  menuPos.value = { x: e.clientX, y: e.clientY };
  menuConnectionId.value = connId;

  nextTick(() => {
    if (connMenuEl.value) {
      const menuRect = connMenuEl.value.getBoundingClientRect();
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;
      
      let x = e.clientX;
      let y = e.clientY;
      
      if (x + menuRect.width > windowWidth) {
        x = windowWidth - menuRect.width - 5;
      }
      
      if (y + menuRect.height > windowHeight) {
        y = windowHeight - menuRect.height - 5;
      }
      
      menuPos.value = { x, y };
    }
  });
}

function closeMenu() {
  menuVisible.value = false;
  menuConnectionId.value = null;
}

function handleEdit() {
  const conn = connectionStore.connections.find((c: any) => c.id === menuConnectionId.value);
  if (conn) {
    connectionStore.openEditConnectionModal(conn);
  }
  closeMenu();
}

async function handleConnect() {
  if (menuConnectionId.value) {
    try {
      await connectionStore.connect(menuConnectionId.value);
    } catch (e) {
      uiStore.showToast('Failed to connect: ' + e, 'error');
    }
  }
  closeMenu();
}

async function handleDisconnect() {
  if (menuConnectionId.value) {
    const id = menuConnectionId.value;
    await connectionStore.disconnect(id);
    // Disconnect already removes from expandedConnectionIds in store
  }
  closeMenu();
}

async function handleDelete() {
  if (menuConnectionId.value && confirm('Are you sure you want to delete this connection?')) {
    await connectionStore.deleteConnection(menuConnectionId.value);
  }
  closeMenu();
}

async function handleRefreshSchemaCache() {
  if (menuConnectionId.value) {
    try {
      await invoke('refresh_schema_cache', { id: menuConnectionId.value });
      uiStore.showToast('Schema cache refreshed successfully.');
    } catch (e) {
      uiStore.showToast('Failed to refresh schema cache: ' + e, 'error');
    }
  }
  closeMenu();
}

async function handleClearSchemaCache() {
  if (menuConnectionId.value) {
    try {
      await invoke('clear_schema_cache', { id: menuConnectionId.value });
      uiStore.showToast('Schema cache cleared.');
    } catch (e) {
      uiStore.showToast('Failed to clear schema cache: ' + e, 'error');
    }
  }
  closeMenu();
}

const objectMenuVisible = ref(false);
const objectMenuPos = ref({ x: 0, y: 0 });
const selectedNode = ref<any>(null);
const objMenuEl = ref<HTMLElement | null>(null);

function handleNodeContextMenu({ event, node, connectionId }: { event: MouseEvent, node: any, connectionId: string }) {
  event.preventDefault();
  event.stopPropagation();
  
  
  menuConnectionId.value = connectionId;
  selectedNode.value = node;
  
  // Set active context based on node type
  if (node.type === 'catalog' || node.type === 'schema') {
    connectionStore.setActiveContext(node.catalog || (node.type === 'catalog' ? node.name : null), node.schema || (node.type === 'schema' ? node.name : null));
  } else if (node.catalog || node.schema) {
    connectionStore.setActiveContext(node.catalog || null, node.schema || null);
  }

  objectMenuPos.value = { x: event.clientX, y: event.clientY };
  objectMenuVisible.value = true;

  nextTick(() => {
    if (objMenuEl.value) {
      const menuRect = objMenuEl.value.getBoundingClientRect();
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;
      
      let x = event.clientX;
      let y = event.clientY;
      
      if (x + menuRect.width > windowWidth) {
        x = windowWidth - menuRect.width - 5;
      }
      
      if (y + menuRect.height > windowHeight) {
        y = windowHeight - menuRect.height - 5;
      }
      
      objectMenuPos.value = { x, y };
    }
  });
}

function handleOpenAiSql() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (connId) {
    tabStore.openAiSqlEditor(connId, selectedNode.value?.catalog, selectedNode.value?.schema);
  }
  closeMenu();
  closeObjectMenu();
}

function handleOpenAiRoutine() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (connId) {
    tabStore.openAiRoutineEditor(connId, selectedNode.value?.catalog, selectedNode.value?.schema);
  }
  closeMenu();
  closeObjectMenu();
}

function handleCreateScript() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (connId) {
    tabStore.createScript(connId, selectedNode.value?.catalog, selectedNode.value?.schema);
  }
  closeMenu();
  closeObjectMenu();
}

function handleCreateObject() {
  if (menuConnectionId.value) {
    tabStore.createTable(menuConnectionId.value, selectedNode.value?.catalog, selectedNode.value?.schema);
  }
  closeObjectMenu();
}

function handleEditObject() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    if (selectedNode.value.type === 'table') {
      tabStore.editTable(connId, selectedNode.value.name, selectedNode.value.catalog, selectedNode.value.schema);
    } else if (selectedNode.value.type === 'view') {
      tabStore.editView(connId, selectedNode.value.name, selectedNode.value.catalog, selectedNode.value.schema);
    } else if (selectedNode.value.type === 'procedure' || selectedNode.value.type === 'function') {
      tabStore.editRoutine(connId, selectedNode.value.name, selectedNode.value.type, selectedNode.value.catalog, selectedNode.value.schema);
    }
  }
  closeObjectMenu();
}

function handleViewHistory() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    const node = selectedNode.value;
    const type = node.type === 'view' ? 'view' : 'routine';
    const tabId = `${type}-${connId}-${node.name}`;
    router.push({ path: '/history', query: { tabId, connectionId: connId, database: node.catalog, schema: node.schema } });
  }
  closeObjectMenu();
}

async function handleRefreshObjects() {
  if (menuConnectionId.value) {
    await connectionStore.refreshObjects(menuConnectionId.value);
  }
  closeObjectMenu();
}

function closeObjectMenu() {
  objectMenuVisible.value = false;
  selectedNode.value = null;
}

onMounted(() => {
  window.addEventListener('click', () => {
    closeMenu();
    closeObjectMenu();
    showAddDropdown.value = false;
    showFooterAddDropdown.value = false;
  });
});

async function handleEmptyTable() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    const table = selectedNode.value;
    const qualifiedName = queryStore.getQualifiedName(connId, table.name, table.catalog, table.schema);
    uiStore.showConfirm(
      'Empty Table',
      `Are you sure you want to EMPTY table "${table.name}"? This will delete ALL rows.`,
      'warning',
      async () => {
        try {
          await queryStore.executeRawQuery(connId, `DELETE FROM ${qualifiedName}`, `Table "${table.name}" emptied successfully.`);
          await connectionStore.refreshObjects(connId);
        } catch (e) {
          // Error already shown by executeRawQuery toast
        }
      }
    );
  }
  closeObjectMenu();
}

async function handleTruncateTable() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    const table = selectedNode.value;
    const qualifiedName = queryStore.getQualifiedName(connId, table.name, table.catalog, table.schema);
    uiStore.showConfirm(
      'Truncate Table',
      `Are you sure you want to TRUNCATE table "${table.name}"? This operation is faster than Empty but might behave differently depending on the database.`,
      'warning',
      async () => {
        try {
          await queryStore.executeRawQuery(connId, `TRUNCATE TABLE ${qualifiedName}`, `Table "${table.name}" truncated successfully.`);
          await connectionStore.refreshObjects(connId);
        } catch (e) {
          // Error already shown by executeRawQuery toast
        }
      }
    );
  }
  closeObjectMenu();
}

async function handleDropTable() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    const table = selectedNode.value;
    const qualifiedName = queryStore.getQualifiedName(connId, table.name, table.catalog, table.schema);
    uiStore.showConfirm(
      'Drop Table',
      `Are you sure you want to DROP table "${table.name}"? THIS CANNOT BE UNDONE AND WILL REMOVE THE TABLE STRUCTURE.`,
      'danger',
      async () => {
        try {
          await queryStore.executeRawQuery(connId, `DROP TABLE ${qualifiedName}`, `Table "${table.name}" dropped successfully.`);
          await connectionStore.refreshObjects(connId);
        } catch (e) {
          // Error already shown by executeRawQuery toast
        }
      }
    );
  }
  closeObjectMenu();
}

function handleViewStructure() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    if (selectedNode.value.type === 'table') {
      tabStore.editTable(connId, selectedNode.value.name, selectedNode.value.catalog, selectedNode.value.schema);
    } else if (selectedNode.value.type === 'view') {
      tabStore.editView(connId, selectedNode.value.name, selectedNode.value.catalog, selectedNode.value.schema);
    } else if (selectedNode.value.type === 'procedure' || selectedNode.value.type === 'function') {
      tabStore.editRoutine(connId, selectedNode.value.name, selectedNode.value.type, selectedNode.value.catalog, selectedNode.value.schema);
    }
  }
  closeObjectMenu();
}

function handleImport() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && connId) {
    invoke('open_import', {
      connectionId: connId,
      catalog: selectedNode.value.catalog,
      schema: selectedNode.value.schema,
      tableName: selectedNode.value.type === 'table' ? selectedNode.value.name : null
    });
  }
  closeObjectMenu();
}

function handleExport() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && selectedNode.value.type === 'table' && connId) {
    invoke('open_export', {
      connectionId: connId,
      sourceType: 'table',
      sourceName: selectedNode.value.name,
      catalog: selectedNode.value.catalog,
      schema: selectedNode.value.schema
    });
  }
  closeObjectMenu();
}

function handleGenerateQuery(type: 'SELECT' | 'INSERT' | 'UPDATE' | 'DELETE') {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && selectedNode.value.type === 'table' && connId) {
    queryStore.generateTableQuery(connId, selectedNode.value.name, type, selectedNode.value.catalog, selectedNode.value.schema);
  }
  closeObjectMenu();
}

function handleViewFirstPage() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && selectedNode.value.type === 'table' && connId) {
    const node = selectedNode.value;
    const tabId = `table-${connId}-${node.name}`;
    tabStore.addTab({
      id: tabId,
      title: node.name,
      type: 'table_data',
      connectionId: connId,
      database: node.catalog,
      schema: node.schema,
      metadata: { tableName: node.name, catalog: node.catalog, schema: node.schema },
      pagination: { page: 1, pageSize: 1000 }
    });
    queryStore.goToFirstPage(tabId);
  }
  closeObjectMenu();
}

async function handleViewLastPage() {
  const connId = menuConnectionId.value || connectionStore.activeConnectionId;
  if (selectedNode.value && selectedNode.value.type === 'table' && connId) {
    const node = selectedNode.value;
    const tabId = `table-${connId}-${node.name}`;
    tabStore.addTab({
      id: tabId,
      title: node.name,
      type: 'table_data',
      connectionId: connId,
      database: node.catalog,
      schema: node.schema,
      metadata: { tableName: node.name, catalog: node.catalog, schema: node.schema },
      pagination: { page: 1, pageSize: 1000 }
    });
    // Need to ensure the tab exists before calling goToLastPage
    await nextTick();
    queryStore.goToLastPage(tabId);
  }
  closeObjectMenu();
}
</script>

<template>
  <aside class="sidebar glass">
    <div class="sidebar-header">
      <div class="brand flex-center gap-2">
        <img src="/app-icon.png" alt="Vi" class="sidebar-logo" />
        <span class="font-bold">ViDBA</span>
        <span class="version-text">v0.5.2</span>
      </div>
    </div>

    <div class="sidebar-content">
      <div class="section">
        <div class="section-title flex-between">
          <span>Connections</span>
          <div class="flex-center gap-1">
            <!-- View mode toggles -->
            <div class="view-mode-bar">
              <button class="vm-btn" :class="{ active: viewMode === 'flat' }" title="Flat list" @click.stop="viewMode = 'flat'">
                <List :size="11" />
              </button>
              <button class="vm-btn" :class="{ active: viewMode === 'product' }" title="Group by project" @click.stop="viewMode = 'product'">
                <Layers :size="11" />
              </button>
              <button class="vm-btn" :class="{ active: viewMode === 'environment' }" title="Group by environment" @click.stop="viewMode = 'environment'">
                <GitBranch :size="11" />
              </button>
              <button class="vm-btn" :class="{ active: viewMode === 'dbtype' }" title="Group by database type" @click.stop="viewMode = 'dbtype'">
                <Database :size="11" />
              </button>
            </div>
            <div class="relative">
            <button class="icon-btn" @click.stop="showAddDropdown = !showAddDropdown">
              <Plus :size="16" />
            </button>
            <div v-if="showAddDropdown" class="dropdown-menu glass anim-scale-in">
              <button class="menu-item" @click="() => handleNewConnection('mySQL')">
                <div class="icon-circle-sm mysql"></div> MySQL
              </button>
              <button class="menu-item" @click="() => handleNewConnection('postgreSQL')">
                <div class="icon-circle-sm postgres"></div> PostgreSQL
              </button>
              <button class="menu-item" @click="() => handleNewConnection('sqlite')">
                <div class="icon-circle-sm sqlite"></div> SQLite
              </button>
              <button class="menu-item" @click="() => handleNewConnection('sqlServer')">
                <div class="icon-circle-sm mssql"></div> SQL Server
              </button>
              <button class="menu-item" @click="() => handleNewConnection('oracle')">
                <div class="icon-circle-sm oracle"></div> Oracle
              </button>
              <button class="menu-item" @click="() => handleNewConnection('mongoDB')">
                <div class="icon-circle-sm mongodb"></div> MongoDB
              </button>
            </div>
          </div>
          </div> <!-- /flex-center gap-1 -->
        </div>

        <div class="sidebar-search-wrapper">
          <Search :size="12" class="search-icon" />
          <input 
            v-model="connectionSearchQuery"
            type="text" 
            placeholder="Filter connections..." 
            class="sidebar-search-input"
          />
        </div>

        <div class="connection-list">
          <div v-if="connectionStore.connections.length === 0" class="empty-state">
            No connections saved
          </div>

          <template v-for="group in connectionGroups" :key="group.key">
            <!-- Group header (hidden in flat mode) -->
            <div
              v-if="viewMode !== 'flat'"
              class="conn-group-header"
              :class="viewMode === 'environment' ? group.key : ''"
              @click="toggleGroup(group.key)"
            >
              <ChevronDown v-if="!collapsedGroups.has(group.key)" :size="11" />
              <ChevronRight v-else :size="11" />
              <span class="group-label">{{ group.label }}</span>
              <span class="group-badge">{{ group.connections.length }}</span>
            </div>

            <!-- Connection items (hidden when group is collapsed) -->
            <template v-if="viewMode === 'flat' || !collapsedGroups.has(group.key)">
              <div
                v-for="conn in group.connections"
                :key="conn.id"
                class="connection-item"
                :class="{
                  active: connectionStore.activeConnectionId === conn.id,
                  'is-grouped': viewMode !== 'flat',
                }"
              >
                <div
                  class="connection-header"
                  @click="toggleConnection(conn.id)"
                  @contextmenu.prevent.stop="showContextMenu($event, conn.id)"
                  :title="`${conn.name}${conn.serverVersion ? '\nVersion: ' + conn.serverVersion : ''}\nType: ${conn.dbType}\n${conn.dbType === 'sqlite' ? 'Path: ' + conn.host : 'Host: ' + conn.host + '\nPort: ' + conn.port + '\nUser: ' + conn.user}${conn.group ? '\nGroup: ' + conn.group : ''}${conn.environment ? '\nEnv: ' + conn.environment : ''}`"
                >
                  <ChevronDown v-if="connectionStore.expandedConnectionIds.has(conn.id)" :size="14" />
                  <ChevronRight v-else :size="14" />
                  <component :is="getConnIcon(conn.dbType)" :size="14" class="type-icon" :class="conn.dbType" />
                  <span class="name">{{ conn.name }}</span>
                  <!-- environment tag (hidden when grouped by env since the group header shows it) -->
                  <span
                    v-if="conn.environment && viewMode !== 'environment'"
                    class="env-tag"
                    :class="conn.environment"
                  >{{ conn.environment }}</span>
                  <div v-if="connectionStore.connectedIds.has(conn.id)" class="status-badge" title="Connected"></div>
                  <button class="icon-btn more-btn" @click.stop="showContextMenu($event, conn.id)">
                    <MoreVertical :size="14" class="more-icon" />
                  </button>
                </div>

                <div v-if="connectionStore.expandedConnectionIds.has(conn.id)" class="connection-objects">
                  <div class="sidebar-search-wrapper mini mx-2 mt-1 mb-1">
                    <Search :size="10" class="search-icon" />
                    <input
                      v-model="connectionNodeSearchQueries[conn.id]"
                      type="text"
                      placeholder="Quick find..."
                      class="sidebar-search-input"
                    />
                  </div>
                  <div v-if="!connectionStore.connectionObjects[conn.id]" class="loading">Loading...</div>
                  <template v-else>
                    <div v-if="connectionTrees[conn.id]?.length === 0" class="empty-objects">No objects found</div>
                    <SidebarTreeNode
                      v-for="node in connectionTrees[conn.id]"
                      :key="node.id"
                      :node="node"
                      :level="0"
                      :connection-id="conn.id"
                      @object-click="handleObjectClick(conn.id, $event)"
                      @open-list="handleOpenList(conn.id, $event)"
                      @node-context-menu="handleNodeContextMenu"
                    />
                  </template>
                </div>
              </div>
            </template>
          </template>
        </div>
      </div>
    </div>

    <div class="sidebar-footer">
      <div class="relative w-full mb-3">
        <button class="button-primary w-full" @click.stop="showFooterAddDropdown = !showFooterAddDropdown">
          <Plus :size="16" />
          New Connection
        </button>
        <div v-if="showFooterAddDropdown" class="dropdown-menu glass anim-scale-in footer-dropdown">
          <button class="menu-item" @click="() => handleNewConnection('mySQL')">
            <div class="icon-circle-sm mysql"></div> MySQL
          </button>
          <button class="menu-item" @click="() => handleNewConnection('postgreSQL')">
            <div class="icon-circle-sm postgres"></div> PostgreSQL
          </button>
          <button class="menu-item" @click="() => handleNewConnection('sqlite')">
            <div class="icon-circle-sm sqlite"></div> SQLite
          </button>
          <button class="menu-item" @click="() => handleNewConnection('sqlServer')">
            <div class="icon-circle-sm mssql"></div> SQL Server
          </button>
          <button class="menu-item" @click="() => handleNewConnection('oracle')">
            <div class="icon-circle-sm oracle"></div> Oracle
          </button>
          <button class="menu-item" @click="() => handleNewConnection('mongoDB')">
            <div class="icon-circle-sm mongodb"></div> MongoDB
          </button>
        </div>
      </div>
      <div class="flex-between">
        <div class="flex gap-2">
          <button class="icon-btn" title="History" @click="invoke('open_history')">
            <History :size="16" />
          </button>
          <button class="icon-btn" title="Settings" @click="handleOpenSettings">
            <Settings :size="16" />
          </button>
        </div>
        <button class="icon-btn" title="Refresh" @click="connectionStore.loadConnections()">
          <RefreshCw :size="16" />
        </button>
      </div>
    </div>

    <!-- Context Menu -->
    <div 
      v-if="menuVisible" 
      ref="connMenuEl"
      class="context-menu glass"
      :style="{ top: menuPos.y + 'px', left: menuPos.x + 'px' }"
      @click.stop
    >
      <button v-if="!connectionStore.connectedIds.has(menuConnectionId!)" class="menu-item" @click="handleConnect">
        <Play :size="14" />
        Connect
      </button>
      <button v-else class="menu-item" @click="handleDisconnect">
        <Power :size="14" />
        Disconnect
      </button>
      <div class="menu-divider"></div>
      <button class="menu-item text-accent" @click="handleOpenAiSql">
        <Sparkles :size="14" />
        Open AI SQL Editor
      </button>
      <div class="menu-divider"></div>
      <button class="menu-item" @click="handleRefreshSchemaCache">
        <RefreshCw :size="14" />
        Refresh Schema Cache
      </button>
      <button class="menu-item" @click="handleClearSchemaCache">
        <Trash2 :size="14" />
        Clear Schema Cache
      </button>
      <div class="menu-divider"></div>
      <button class="menu-item" @click="handleEdit">
        <Edit2 :size="14" />
        Edit
      </button>
      <button class="menu-item text-error" @click="handleDelete">
        <Trash2 :size="14" />
        Delete
      </button>
    </div>

    <div 
      v-if="objectMenuVisible" 
      ref="objMenuEl"
      class="context-menu glass"
      :style="{ top: objectMenuPos.y + 'px', left: objectMenuPos.x + 'px' }"
      @click.stop
    >
      <template v-if="selectedNode?.type === 'catalog' || selectedNode?.type === 'schema' || selectedNode?.type === 'category'">
        <button class="menu-item" @click="handleCreateScript">
          <Plus :size="14" />
          New Script
        </button>
        <button class="menu-item text-accent" @click="handleOpenAiSql">
          <Sparkles :size="14" />
          AI SQL Editor
        </button>
        <div class="menu-divider"></div>
      </template>

      <template v-if="selectedNode?.type === 'category' && selectedNode?.name === 'Tables'">
        <button class="menu-item" @click="handleCreateObject">
          <Plus :size="14" />
          Create Table
        </button>
        <button class="menu-item" @click="handleImport">
          <Upload :size="14" />
          Import Data
        </button>
        <button class="menu-item" @click="handleRefreshObjects">
          <RefreshCw :size="14" />
          Refresh
        </button>
      </template>

      <template v-if="selectedNode?.type === 'category' && (selectedNode?.name === 'Procedures' || selectedNode?.name === 'Functions')">
        <button class="menu-item text-accent" @click="handleOpenAiRoutine">
          <Sparkles :size="14" />
          Open AI Routine Editor
        </button>
        <div class="menu-divider"></div>
      </template>

      <template v-if="['table', 'view', 'procedure', 'function'].includes(selectedNode?.type)">
        <template v-if="selectedNode?.type === 'table' || selectedNode?.type === 'view'">
          <div class="menu-item has-submenu">
            <FileText :size="14" />
            <span>Query...</span>
            <ChevronRight :size="14" class="ml-auto opacity-50" />
            <div class="submenu glass">
              <button class="menu-item" @click="handleGenerateQuery('SELECT')">
                <Search :size="14" />
                SELECT
              </button>
              <button class="menu-item" @click="handleGenerateQuery('INSERT')">
                <Plus :size="14" />
                INSERT
              </button>
              <button class="menu-item" @click="handleGenerateQuery('UPDATE')">
                <Edit2 :size="14" />
                UPDATE
              </button>
              <button class="menu-item" @click="handleGenerateQuery('DELETE')">
                <Trash2 :size="14" />
                DELETE
              </button>
            </div>
          </div>
          <div class="menu-divider"></div>
        </template>
        
        <template v-if="selectedNode?.type === 'table'">
          <button class="menu-item" @click="handleViewFirstPage">
            <ChevronsLeft :size="14" />
            View First Page
          </button>
          <button class="menu-item" @click="handleViewLastPage">
            <ChevronsRight :size="14" />
            View Last Page
          </button>
          <div class="menu-divider"></div>
          <button class="menu-item" @click="handleViewStructure">
            <Columns :size="14" />
            View Structure
          </button>
        </template>
        <button class="menu-item" @click="handleEditObject">
          <Edit2 :size="14" />
          Edit {{ selectedNode.type === 'table' ? 'Table' : (selectedNode.type === 'view' ? 'View' : 'Routine') }}
        </button>
        
        <button v-if="['procedure', 'function', 'view'].includes(selectedNode?.type)" class="menu-item" @click="handleViewHistory">
          <Clock :size="14" />
          View History
        </button>
        
        <div v-if="selectedNode?.type === 'table'" class="menu-divider"></div>
        <button v-if="selectedNode?.type === 'table'" class="menu-item" @click="handleImport">
          <Upload :size="14" />
          Import
        </button>
        <button v-if="selectedNode?.type === 'table'" class="menu-item" @click="handleExport">
          <Download :size="14" />
          Export
        </button>
        
        <div v-if="selectedNode?.type === 'table'" class="menu-divider"></div>
        <button v-if="selectedNode?.type === 'table'" class="menu-item text-warning" @click="handleEmptyTable">
          <Trash2 :size="14" />
          Empty Table
        </button>
        <button v-if="selectedNode?.type === 'table'" class="menu-item text-warning" @click="handleTruncateTable">
          <Trash2 :size="14" />
          Truncate Table
        </button>
        <button v-if="selectedNode?.type === 'table'" class="menu-item text-error" @click="handleDropTable">
          <Trash2 :size="14" />
          Drop Table
        </button>
      </template>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-width);
  height: 100%;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-color);
  z-index: 50;
}

.sidebar-header {
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-color);
}

.brand {
  font-size: 0.95rem;
  letter-spacing: -0.01em;
  line-height: 1.1;
}

.version-text {
  font-size: 0.65rem;
  opacity: 0.5;
  font-weight: 400;
}

.text-accent {
  color: var(--accent-primary);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
}

.section {
  margin-bottom: 24px;
}

.section-title {
  padding: 0 12px;
  margin-bottom: 4px;
  font-size: 0.7rem;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-secondary);
  letter-spacing: 0.05em;
}

.connection-item {
  margin-bottom: 2px;
}

.connection-header {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  gap: 6px;
  cursor: pointer;
  transition: background 0.2s;
  font-size: 0.85rem;
  overflow: hidden; /* Ensure it doesn't break layout */
}

.connection-header .name {
  white-space: nowrap;
  overflow-x: auto;
  flex: 1;
  min-width: 0;
  scrollbar-width: none;
}

.connection-header .name::-webkit-scrollbar {
  display: none;
}

.connection-header:hover {
  background: var(--glass-border);
}

.connection-item.active .connection-header {
  background: rgba(59, 130, 246, 0.1);
  color: var(--accent-primary);
}

.type-icon {
  opacity: 0.7;
}

.type-icon.mySQL { color: #f29111; }
.type-icon.postgreSQL { color: #33a9dc; }
.type-icon.sqlite { color: #00b0ed; }
.type-icon.sqlServer { color: #eb2d35; }
.type-icon.oracle { color: #f00; }
.type-icon.mongoDB { color: #47a248; }

.status-badge {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: #10b981;
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.4);
}


.font-bold {
  font-weight: 700;
}

.more-btn {
  margin-left: auto;
  padding: 2px;
  opacity: 0;
  transition: opacity 0.2s;
}

.connection-header:hover .more-btn {
  opacity: 0.6;
}

.more-btn:hover {
  opacity: 1 !important;
  background: var(--glass-border);
}

.sidebar-search-wrapper {
  padding: 0 12px;
  margin-bottom: 8px;
  position: relative;
  display: flex;
  align-items: center;
}

.sidebar-search-input {
  width: 100%;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 4px 8px 4px 26px;
  font-size: 0.75rem;
  color: var(--text-primary);
  outline: none;
  transition: all 0.2s;
}

.sidebar-search-input:focus {
  border-color: var(--accent-primary);
  background: var(--bg-primary);
}

.sidebar-search-wrapper .search-icon {
  position: absolute;
  left: 20px;
  color: var(--text-secondary);
  pointer-events: none;
  opacity: 0.6;
}

.context-menu {
  position: fixed;
  min-width: 180px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  box-shadow: var(--shadow-lg);
  z-index: 1000;
  display: flex;
  flex-direction: column;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: 0.85rem;
  border-radius: 4px;
  cursor: pointer;
  text-align: left;
  transition: all 0.2s;
  width: 100%;
}

.menu-item:hover {
  background: var(--glass-border);
}

.menu-divider {
  height: 1px;
  background: var(--border-color);
  margin: 4px;
}

.has-submenu {
  position: relative;
  display: flex !important;
  align-items: center;
  gap: 8px;
}

.submenu {
  position: absolute;
  top: 0;
  left: 100%;
  min-width: 140px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  z-index: 1001;
  box-shadow: var(--shadow-lg);
  display: none;
  margin-left: 2px;
}

.has-submenu:hover > .submenu {
  display: block;
}

.ml-auto { margin-left: auto; }
.opacity-50 { opacity: 0.5; }

.menu-item.text-error {
  color: #f87171;
}

.menu-item.text-error:hover {
  background: rgba(248, 113, 113, 0.1);
}

.connection-objects {
  padding-left: 8px; /* Reduced for hierarchy */
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.empty-objects {
  padding: 8px 16px;
  font-size: 0.75rem;
  opacity: 0.5;
}

.sidebar-footer {
  padding: 16px;
  border-top: 1px solid var(--border-color);
}

.icon-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  display: flex;
  transition: all 0.2s;
}

.icon-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.empty-state {
  padding: 16px;
  font-size: 0.8rem;
  color: var(--text-secondary);
  text-align: center;
}

.loading {
  padding: 8px;
  font-size: 0.75rem;
  opacity: 0.5;
}

.w-full {
  width: 100%;
}

.relative { position: relative; }

.dropdown-menu {
  position: absolute;
  top: 100%;
  right: 0;
  min-width: 160px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  z-index: 1000;
  box-shadow: var(--shadow-lg);
  margin-top: 4px;
}

.icon-circle-sm {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 8px;
}

.mysql { background: #f29111; }
.postgres { background: #33a9dc; }
.mssql { background: #eb2d35; }
.sqlite { background: #00b0ed; }
.oracle { background: #f00; }
.mongodb { background: #47a248; }

.text-warning { color: #f59e0b; }

.dropdown-menu.footer-dropdown {
  top: auto;
  bottom: 100%;
  margin-top: 0;
  margin-bottom: 8px;
}

.anim-scale-in {
  animation: scaleIn 0.15s ease-out;
}

@keyframes scaleIn {
  from { opacity: 0; transform: scale(0.95) translateY(-10px); }
  to { opacity: 1; transform: scale(1) translateY(0); }
}

.gap-2 {
  gap: 8px;
}

.mb-3 {
  margin-bottom: 12px;
}

.sidebar-logo {
  width: 18px;
  height: 18px;
  object-fit: contain;
}

.sidebar-search-wrapper.mini {
  margin-bottom: 4px;
}

.sidebar-search-wrapper.mini .sidebar-search-input {
  padding: 2px 6px 2px 22px;
  font-size: 0.7rem;
}

.sidebar-search-wrapper.mini .search-icon {
  left: 18px;
}

.mx-2 { margin-left: 8px; margin-right: 8px; }
.mt-1 { margin-top: 4px; }
.mb-1 { margin-bottom: 4px; }

/* ── View mode toggle bar ─────────────────────────────────────────────── */
.view-mode-bar {
  display: flex;
  background: var(--bg-tertiary);
  border-radius: 4px;
  padding: 2px;
  gap: 1px;
}

.vm-btn {
  background: transparent;
  border: none;
  padding: 3px 5px;
  border-radius: 3px;
  cursor: pointer;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  transition: all 0.15s;
}
.vm-btn:hover { color: var(--text-primary); background: var(--glass-border); }
.vm-btn.active { color: var(--accent-primary); background: var(--bg-secondary); }

/* ── Connection group header ──────────────────────────────────────────── */
.conn-group-header {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 10px 4px;
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  border-top: 1px solid var(--border-color);
  margin-top: 2px;
  transition: color 0.15s;
}
.conn-group-header:first-child { border-top: none; margin-top: 0; }
.conn-group-header:hover { color: var(--text-primary); }

/* environment-coloured group headers */
.conn-group-header.dev         { color: #60a5fa; }
.conn-group-header.test        { color: #fbbf24; }
.conn-group-header.staging,
.conn-group-header.qa,
.conn-group-header.uat         { color: #fb923c; }
.conn-group-header.beta        { color: #a78bfa; }
.conn-group-header.prod,
.conn-group-header.production  { color: #f87171; }

.group-label { flex: 1; }
.group-badge {
  font-size: 0.65rem;
  font-weight: 600;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 0 5px;
  opacity: 0.7;
  text-transform: none;
  letter-spacing: 0;
}

/* ── Indentation for grouped connection items ─────────────────────────── */
.connection-item.is-grouped > .connection-header {
  padding-left: 18px;
}

/* ── Environment tag in connection header ─────────────────────────────── */
.env-tag {
  flex-shrink: 0;
  font-size: 0.58rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  padding: 1px 5px;
  border-radius: 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border: 1px solid var(--border-color);
}
.env-tag.dev         { background: rgba(59,130,246,0.12);  color: #60a5fa;  border-color: rgba(59,130,246,0.25); }
.env-tag.test        { background: rgba(234,179,8,0.12);   color: #fbbf24;  border-color: rgba(234,179,8,0.25); }
.env-tag.staging,
.env-tag.qa,
.env-tag.uat         { background: rgba(249,115,22,0.12);  color: #fb923c;  border-color: rgba(249,115,22,0.25); }
.env-tag.beta        { background: rgba(139,92,246,0.12);  color: #a78bfa;  border-color: rgba(139,92,246,0.25); }
.env-tag.prod,
.env-tag.production  { background: rgba(239,68,68,0.12);   color: #f87171;  border-color: rgba(239,68,68,0.25); }
</style>
