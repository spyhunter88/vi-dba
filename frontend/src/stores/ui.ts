import { defineStore } from 'pinia';
import { ref, watch, computed } from 'vue';
import { emit as tauriEmit, listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

export interface Toast {
  id: string;
  message: string;
  type: 'success' | 'error' | 'info' | 'warning';
  duration?: number;
}

export const useUiStore = defineStore('ui', () => {
  const toasts = ref<Toast[]>([]);
  const theme = ref<'light' | 'dark' | 'system'>('dark');
  const confirmState = ref<{
    show: boolean;
    title: string;
    message: string;
    type: 'danger' | 'warning' | 'info';
    onConfirm: (() => void) | null;
  }>({
    show: false,
    title: '',
    message: '',
    type: 'warning',
    onConfirm: null
  });

  const historyPopupState = ref<{
    show: boolean;
    entry: any | null;
  }>({
    show: false,
    entry: null
  });

  const systemPreferredTheme = ref<'light' | 'dark'>(
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  );

  const resolvedTheme = computed(() => {
    if (theme.value === 'system') return systemPreferredTheme.value;
    return theme.value;
  });

  function showToast(message: string, type: Toast['type'] = 'success', duration = 3000) {
    const id = Math.random().toString(36).substring(2, 9);
    const toast: Toast = { id, message, type, duration };
    toasts.value.push(toast);

    if (duration > 0) {
      setTimeout(() => {
        removeToast(id);
      }, duration);
    }
  }

  function removeToast(id: string) {
    toasts.value = toasts.value.filter(t => t.id !== id);
  }

  function showConfirm(title: string, message: string, type: 'danger' | 'warning' | 'info', onConfirm: () => void) {
    confirmState.value = { show: true, title, message, type, onConfirm };
  }

  function setTheme(newTheme: 'light' | 'dark' | 'system') {
    console.log('[Theme] setTheme:', newTheme);
    theme.value = newTheme;
    localStorage.setItem('theme', newTheme);
    tauriEmit('vi-theme-update', newTheme);
  }

  function applyTheme(t: 'light' | 'dark' | 'system') {
    console.log('[Theme] Applying theme:', t);
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

    if (t === 'dark') {
      document.documentElement.classList.add('dark');
      document.documentElement.classList.remove('light');
    } else if (t === 'light') {
      document.documentElement.classList.add('light');
      document.documentElement.classList.remove('dark');
    } else {
      const isDark = mediaQuery.matches;
      document.documentElement.classList.toggle('dark', isDark);
      document.documentElement.classList.toggle('light', !isDark);
    }

    // Always keep systemPreferredTheme updated and listener attached
    const listener = (e: MediaQueryListEvent) => {
      systemPreferredTheme.value = e.matches ? 'dark' : 'light';
      if (theme.value === 'system') {
        document.documentElement.classList.toggle('dark', e.matches);
        document.documentElement.classList.toggle('light', !e.matches);
      }
    };
    mediaQuery.removeEventListener('change', listener);
    mediaQuery.addEventListener('change', listener);
  }

  async function initTheme() {
    console.log('[Theme] Initializing theme store...');
    const saved = localStorage.getItem('theme') as 'light' | 'dark' | 'system';
    if (saved && saved !== theme.value) {
      theme.value = saved;
    }

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    systemPreferredTheme.value = mediaQuery.matches ? 'dark' : 'light';

    await listen<'light' | 'dark' | 'system'>('vi-theme-update', (event) => {
      if (event.payload !== theme.value) {
        theme.value = event.payload;
      }
    });

    window.addEventListener('storage', (e) => {
      if (e.key === 'theme') {
        const newTheme = e.newValue as 'light' | 'dark' | 'system';
        if (newTheme && newTheme !== theme.value) {
          theme.value = newTheme;
        }
      }
    });
  }

  async function copyToClipboard(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      showToast('Copied to clipboard');
    } catch (e) {
      console.error('Failed to copy info:', e);
    }
  }

  watch(theme, (newVal) => {
    applyTheme(newVal);
  }, { immediate: true });

  async function openHistoryPopup(entry: any) {
    historyPopupState.value = {
      show: true,
      entry
    };

    const label = 'history-detail';
    const webview = await WebviewWindow.getByLabel(label);

    if (webview) {
      await webview.setFocus();
      await tauriEmit('vi-history-detail-update', entry);
    } else {
      new WebviewWindow(label, {
        url: '/history-detail',
        title: 'Query History Detail',
        width: 600,
        height: 700,
        resizable: true,
        decorations: true,
        center: true,
        alwaysOnTop: false,
      });

      // Wait for window to load then emit
      // Or listen for the window to be ready
      listen('vi-history-detail-ready', () => {
        if (historyPopupState.value.entry) {
          tauriEmit('vi-history-detail-update', historyPopupState.value.entry);
        }
      });
    }
  }

  function closeHistoryPopup() {
    historyPopupState.value.show = false;
  }

  return {
    toasts,
    theme,
    resolvedTheme,
    confirmState,
    historyPopupState,
    showToast,
    removeToast,
    showConfirm,
    setTheme,
    initTheme,
    copyToClipboard,
    openHistoryPopup,
    closeHistoryPopup
  };
});
