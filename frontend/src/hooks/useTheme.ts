import { useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { applyTheme, getPresetByName } from '../themes';

export function useTheme() {
  const themeName = useAppStore(s => s.themeName);

  useEffect(() => {
    const preset = getPresetByName(themeName);
    if (preset) {
      applyTheme(preset);
    }
  }, [themeName]);

  return { themeName };
}
