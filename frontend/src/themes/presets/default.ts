import { ThemePreset } from '../../types/theme';

export const defaultTheme: ThemePreset = {
  name: 'Default',
  description: 'Dark theme with blue accents',
  colors: {
    bg: '#1e1e2e',
    bgSecondary: '#313244',
    bgOverlay: 'rgba(0, 0, 0, 0.5)',
    textPrimary: '#cdd6f4',
    textSecondary: '#a6adc8',
    textMuted: '#6c7086',
    border: '#45475a',
    accent: '#89b4fa',
    success: '#a6e3a1',
    error: '#f38ba8',
    warning: '#f9e2af',
  },
};
