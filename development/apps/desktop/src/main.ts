import './styles/workbench.css';
import { Application } from './app/application.ts';
import { requiredElement } from './components/html.ts';
import metadata from '../package.json';

// Lazy access also handles environments in which obtaining localStorage throws.
new Application(requiredElement<HTMLElement>('#app'), {
  getItem: key => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value),
}, metadata.version);
