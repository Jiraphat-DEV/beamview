import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

// Set theme synchronously before Svelte mounts to avoid a light-mode
// flash on dark systems. The theme store (loaded from config) will
// overwrite this attribute later if the user has an explicit pref.
document.documentElement.dataset.theme = window.matchMedia('(prefers-color-scheme: dark)').matches
  ? 'dark'
  : 'light';

const app = mount(App, {
  target: document.getElementById('app')!,
});

export default app;
