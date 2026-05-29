import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dialog } from '../ui/Dialog';
import { useUIStore } from '../../stores/uiStore';
import { themePresets, applyTheme } from '../../themes';

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SettingsDialog: React.FC<SettingsDialogProps> = ({ isOpen, onClose }) => {
  const { i18n } = useTranslation();
  const { themeName, setThemeName } = useUIStore();

  const currentLang = i18n.language || 'en';

  const handleLanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
  };

  const handleThemeChange = (name: string) => {
    setThemeName(name);
    const preset = themePresets.find((p) => p.name === name);
    if (preset) applyTheme(preset);
  };

  return (
    <Dialog isOpen={isOpen} onClose={onClose} title="设置">
      <div className="space-y-6">
        {/* 语言选择 */}
        <div>
          <label className="text-sm font-medium text-text-secondary mb-2 block">
            语言 / Language
          </label>
          <div className="flex gap-2">
            <button
              onClick={() => handleLanguageChange('zh')}
              className={
                currentLang === 'zh'
                  ? 'bg-brand text-white rounded-lg px-4 py-2 text-sm transition-colors'
                  : 'bg-bg-overlay text-text-muted rounded-lg px-4 py-2 text-sm hover:bg-bg transition-colors'
              }
            >
              🇨🇳 中文
            </button>
            <button
              onClick={() => handleLanguageChange('en')}
              className={
                currentLang === 'en'
                  ? 'bg-brand text-white rounded-lg px-4 py-2 text-sm transition-colors'
                  : 'bg-bg-overlay text-text-muted rounded-lg px-4 py-2 text-sm hover:bg-bg transition-colors'
              }
            >
              🇬🇧 English
            </button>
          </div>
        </div>

        {/* 主题选择 */}
        <div>
          <label className="text-sm font-medium text-text-secondary mb-2 block">
            主题 / Theme
          </label>
          <select
            value={themeName}
            onChange={(e) => handleThemeChange(e.target.value)}
            className="w-full bg-bg border border-border rounded-lg px-3 py-2 text-sm text-text focus:outline-none focus:border-brand"
          >
            {themePresets.map((preset) => (
              <option key={preset.name} value={preset.name}>
                {preset.name}
              </option>
            ))}
          </select>
        </div>
      </div>
    </Dialog>
  );
};
