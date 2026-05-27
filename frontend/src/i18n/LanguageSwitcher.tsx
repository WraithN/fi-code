import React from 'react';
import { useTranslation } from 'react-i18next';

export const LanguageSwitcher: React.FC = () => {
  const { i18n } = useTranslation();

  const toggleLanguage = () => {
    const next = i18n.language === 'zh' ? 'en' : 'zh';
    i18n.changeLanguage(next);
  };

  return (
    <button
      onClick={toggleLanguage}
      className="p-2 rounded-lg hover:bg-bg-overlay transition-colors text-sm text-text-muted"
      title={i18n.language === 'zh' ? 'Switch to English' : '切换到中文'}
    >
      {i18n.language === 'zh' ? '🇨🇳 中文' : '🇬🇧 EN'}
    </button>
  );
};
