<script setup lang="ts">
import { computed } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import MySqlTableList from './table-list/MySqlTableList.vue';
import PostgresTableList from './table-list/PostgresTableList.vue';
import SqliteTableList from './table-list/SqliteTableList.vue';
import SqlServerTableList from './table-list/SqlServerTableList.vue';
import GenericTableList from './table-list/GenericTableList.vue';

const props = defineProps<{ tabId: string }>();

const tabStore = useTabStore();
const connectionStore = useConnectionStore();

const dbType = computed(() => {
  const tab = tabStore.tabs.find(t => t.id === props.tabId);
  const conn = connectionStore.connections.find(c => c.id === tab?.connectionId);
  return conn?.dbType;
});
</script>

<template>
  <MySqlTableList      v-if="dbType === 'mySQL'"       :tab-id="tabId" />
  <PostgresTableList   v-else-if="dbType === 'postgreSQL'" :tab-id="tabId" />
  <SqliteTableList     v-else-if="dbType === 'sqlite'"  :tab-id="tabId" />
  <SqlServerTableList  v-else-if="dbType === 'sqlServer'" :tab-id="tabId" />
  <GenericTableList    v-else                           :tab-id="tabId" />
</template>
