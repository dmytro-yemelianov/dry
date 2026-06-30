import DefaultTheme from 'vitepress/theme';
import type { Theme } from 'vitepress';

export default {
  extends: DefaultTheme,
  enhanceApp() {
    // <LiveExample> is registered here in Task 6.
  },
} satisfies Theme;
