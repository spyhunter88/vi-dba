<script setup lang="ts">
import { ref, computed } from 'vue';
import { 
  ChevronRight, 
  ChevronDown, 
  Table as TableIcon, 
  Eye, 
  FunctionSquare,
  Box,
  Layers,
  Database
} from 'lucide-vue-next';

interface TreeNode {
  id: string;
  name: string;
  type: 'catalog' | 'schema' | 'category' | 'table' | 'view' | 'function' | 'procedure';
  catalog?: string;
  schema?: string;
  children?: TreeNode[];
}

const props = defineProps<{
  node: TreeNode;
  level: number;
  connectionId: string;
}>();

const emit = defineEmits(['object-click', 'open-list', 'node-context-menu']);

const isExpanded = ref(false);

const hasChildren = computed(() => props.node.children && props.node.children.length > 0);

function toggle() {
  if (hasChildren.value) {
    isExpanded.value = !isExpanded.value;
  }
  
  // Also emit open-list if it's a relevant node
  if (props.node.type === 'category' && (props.node.name === 'Tables' || props.node.name === 'Procedures' || props.node.name === 'Functions')) {
    emit('open-list', props.node);
  } else if (props.node.type === 'catalog' || props.node.type === 'schema') {
    emit('open-list', props.node);
  } else if (props.node.type === 'table' || props.node.type === 'view' || props.node.type === 'function' || props.node.type === 'procedure') {
    // Single click to open table/view data or routine/view editor
    emit('object-click', {
      name: props.node.name,
      objectType: props.node.type,
      catalog: props.node.catalog,
      schema: props.node.schema
    });
  }
}


function handleContextMenu(e: MouseEvent) {
  emit('node-context-menu', { event: e, node: props.node, connectionId: props.connectionId });
}

const icon = computed(() => {
  switch (props.node.type) {
    case 'catalog': return Database;
    case 'schema': return Layers;
    case 'category': return Box;
    case 'table': return TableIcon;
    case 'view': return Eye;
    default: return FunctionSquare;
  }
});

const tooltip = computed(() => {
  if (props.node.type === 'table' || props.node.type === 'view' ||
      props.node.type === 'function' || props.node.type === 'procedure') {
    return props.node.name;
  }
  return undefined;
});
</script>

<template>
  <div class="tree-node">
    <div
      class="node-header"
      :style="{ paddingLeft: (level * 12 + 8) + 'px' }"
      :title="tooltip"
      @click="toggle"
      @contextmenu.prevent="handleContextMenu"
    >
      <span class="expander" v-if="hasChildren">
        <ChevronDown v-if="isExpanded" :size="12" />
        <ChevronRight v-else :size="12" />
      </span>
      <span v-else class="expander-spacer"></span>

      <component :is="icon" :size="14" class="node-icon" :class="node.type" />
      <span class="node-name">{{ node.name }}</span>
    </div>

    <div v-if="isExpanded && hasChildren" class="node-children">
      <SidebarTreeNode 
        v-for="child in node.children" 
        :key="child.id" 
        :node="child" 
        :level="level + 1"
        :connection-id="connectionId"
        @object-click="$emit('object-click', $event)"
        @open-list="$emit('open-list', $event)"
        @node-context-menu="$emit('node-context-menu', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.node-header {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 0;
  cursor: pointer;
  font-size: 0.8rem;
  color: var(--text-primary);
  white-space: nowrap;
  transition: background 0.1s;
}

.node-header:hover {
  background: var(--glass-border);
}

.expander {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  color: var(--text-secondary);
}

.expander-spacer {
  width: 16px;
}

.node-icon {
  opacity: 0.7;
  flex-shrink: 0;
}

.node-icon.catalog { color: var(--accent-primary); }
.node-icon.schema { color: #a855f7; }
.node-icon.category { color: var(--text-warning); opacity: 0.5; }
.node-icon.table { color: var(--accent-primary); }
.node-icon.view { color: var(--text-success); }

.node-name {
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
