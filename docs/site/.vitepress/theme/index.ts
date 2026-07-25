import DefaultTheme from 'vitepress/theme';
import type { Theme } from 'vitepress';
import LiveExample from '@dry-live-example';
import './style.css';

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('LiveExample', LiveExample);
  },
} satisfies Theme;
