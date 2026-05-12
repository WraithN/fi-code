import { ThemePreset } from '../types/theme';
import { defaultTheme } from './presets/default';
import { lightTheme } from './presets/light';
import { monokaiTheme } from './presets/monokai';

export const themePresets: ThemePreset[] = [defaultTheme, lightTheme, monokaiTheme];

export function applyTheme(preset: ThemePreset): void {
  const root = document.documentElement;
  root.style.setProperty('--color-bg', preset.colors.bg);
  root.style.setProperty('--color-bg-secondary', preset.colors.bgSecondary);
  root.style.setProperty('--color-bg-overlay', preset.colors.bgOverlay);
  root.style.setProperty('--color-text-primary', preset.colors.textPrimary);
  root.style.setProperty('--color-text-secondary', preset.colors.textSecondary);
  root.style.setProperty('--color-text-muted', preset.colors.textMuted);
  root.style.setProperty('--color-border', preset.colors.border);
  root.style.setProperty('--color-accent', preset.colors.accent);
  root.style.setProperty('--color-success', preset.colors.success);
  root.style.setProperty('--color-error', preset.colors.error);
  root.style.setProperty('--color-warning', preset.colors.warning);
}

export function getPresetByName(name: string): ThemePreset | undefined {
  return themePresets.find(p => p.name === name);
}
