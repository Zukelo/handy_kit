import '../../../design-system/tokens.css';
import './styles/workbench.css';
import '../../../design-system/components.css';
import { Application } from './app/application.ts';
import { requiredElement } from './components/html.ts';
import metadata from '../package.json';

// Lazy access also handles environments in which obtaining localStorage throws.
new Application(requiredElement<HTMLElement>('#app'), {
  getItem: key => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value),
}, metadata.version);

// Keep native window chrome in sync with the selected semantic appearance.
import { invoke, isTauri } from '@tauri-apps/api/core';
function applyNativeAppearance(): void {
  if (isTauri()) void invoke('set_appearance', { mode: document.documentElement.dataset.appearance ?? 'system' }).catch(error => console.warn('Native appearance unavailable', error));
}
document.addEventListener('handykit:appearance', applyNativeAppearance);
applyNativeAppearance();
