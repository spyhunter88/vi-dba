<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import { FileText, ChevronRight, Search, Plus, Edit2, Trash2, ChevronsLeft, ChevronsRight, Columns, Upload, Download } from 'lucide-vue-next';

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
}>();

const emit = defineEmits<{
  close: [];
  generateQuery: [type: 'SELECT' | 'INSERT' | 'UPDATE' | 'DELETE'];
  viewFirst: [];
  viewLast: [];
  viewStructure: [];
  editTable: [];
  import: [];
  export: [];
  empty: [];
  truncate: [];
  drop: [];
}>();

const menuEl = ref<HTMLElement | null>(null);
const adjX = ref(0);
const adjY = ref(0);
const ready = ref(false);

// Single watch on all three props — always recomputes position with bounds check.
// Menu is hidden (opacity:0, pointer-events:none) until position is finalised,
// preventing a flash at the raw cursor position before flipping.
watch(
  [() => props.visible, () => props.x, () => props.y],
  async ([visible]) => {
    if (!visible) { ready.value = false; return; }
    ready.value = false;
    adjX.value = props.x;
    adjY.value = props.y;
    await nextTick();
    if (menuEl.value) {
      const { width, height } = menuEl.value.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      // Flip left when not enough room to the right
      adjX.value = props.x + width > vw ? Math.max(0, props.x - width) : props.x;
      // Flip above when not enough room below
      adjY.value = props.y + height > vh ? Math.max(0, props.y - height) : props.y;
    }
    ready.value = true;
  }
);
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuEl"
      class="ctx-menu glass"
      :class="{ ready }"
      :style="{ top: adjY + 'px', left: adjX + 'px' }"
      @click.stop
    >
      <div class="ctx-item has-sub">
        <FileText :size="14" />
        <span>Query…</span>
        <ChevronRight :size="12" class="ml-auto opacity-50" />
        <div class="submenu glass">
          <button class="ctx-item" @click="emit('generateQuery', 'SELECT')"><Search :size="13" />SELECT</button>
          <button class="ctx-item" @click="emit('generateQuery', 'INSERT')"><Plus :size="13" />INSERT</button>
          <button class="ctx-item" @click="emit('generateQuery', 'UPDATE')"><Edit2 :size="13" />UPDATE</button>
          <button class="ctx-item" @click="emit('generateQuery', 'DELETE')"><Trash2 :size="13" />DELETE</button>
        </div>
      </div>
      <div class="ctx-divider"></div>
      <button class="ctx-item" @click="emit('viewFirst')"><ChevronsLeft :size="14" />View First Page</button>
      <button class="ctx-item" @click="emit('viewLast')"><ChevronsRight :size="14" />View Last Page</button>
      <div class="ctx-divider"></div>
      <button class="ctx-item" @click="emit('viewStructure')"><Columns :size="14" />View Structure</button>
      <button class="ctx-item" @click="emit('editTable')"><Edit2 :size="14" />Edit Table</button>
      <button class="ctx-item" @click="emit('import')"><Upload :size="14" />Import…</button>
      <button class="ctx-item" @click="emit('export')"><Download :size="14" />Export…</button>
      <div class="ctx-divider"></div>
      <button class="ctx-item warn" @click="emit('empty')"><Trash2 :size="14" />Empty Table</button>
      <button class="ctx-item warn" @click="emit('truncate')"><Trash2 :size="14" />Truncate Table</button>
      <button class="ctx-item danger" @click="emit('drop')"><Trash2 :size="14" />Drop Table</button>
    </div>
  </Teleport>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  min-width: 180px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  z-index: 3000;
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  /* Hidden until position is computed to avoid flash at raw cursor position */
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.08s ease;
}

.ctx-menu.ready {
  opacity: 1;
  pointer-events: auto;
}

.ctx-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 7px 11px;
  background: transparent;
  border: none;
  border-radius: 5px;
  color: var(--text-primary);
  font-size: 0.82rem;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s;
}

.ctx-item:hover { background: var(--glass-border); }
.ctx-item.warn  { color: var(--text-warning, #f59e0b); }
.ctx-item.danger { color: #f87171; }
.ctx-item.warn:hover  { background: rgba(245, 158, 11, 0.08); }
.ctx-item.danger:hover { background: rgba(248, 113, 113, 0.08); }

.ctx-divider {
  height: 1px;
  background: var(--border-color);
  margin: 3px 4px;
}

.has-sub {
  position: relative;
  display: flex !important;
  cursor: default;
}

.submenu {
  position: absolute;
  top: 0;
  left: 100%;
  min-width: 130px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  z-index: 3001;
  box-shadow: var(--shadow-lg);
  display: none;
  margin-left: 2px;
}

.has-sub:hover > .submenu { display: flex; flex-direction: column; }

.ml-auto { margin-left: auto; }
.opacity-50 { opacity: 0.5; }
</style>
