export interface ThemeColors {
  bg: string;
  bgSecondary: string;
  bgOverlay: string;
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  border: string;
  accent: string;
  success: string;
  error: string;
  warning: string;
}

export interface ThemePreset {
  name: string;
  description: string;
  colors: ThemeColors;
}
