import { ThemePreset } from '../../types/theme';

export const monokaiTheme: ThemePreset = {
  name: 'Monokai',
  description: 'High contrast dark',
  colors: {
    bg: '#272822',
    bgSecondary: '#3e3d32',
    bgOverlay: 'rgba(0, 0, 0, 0.5)',
    textPrimary: '#f8f8f2',
    textSecondary: '#a59f85',
    textMuted: '#75715e',
    border: '#49483e',
    accent: '#66d9ef',
    success: '#a6e22e',
    error: '#f92672',
    warning: '#fd971f',
  },
};
