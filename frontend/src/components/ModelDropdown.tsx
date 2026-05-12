import React, { useState } from 'react';
import { ProviderItem, ModelItem } from '../types/api';

interface ModelDropdownProps {
  providers: ProviderItem[];
  currentModel: string;
  onSelect: (provider: string, model: string, needsKey: boolean) => void;
  onClose: () => void;
}

export const ModelDropdown: React.FC<ModelDropdownProps> = ({
  providers,
  currentModel,
  onSelect,
  onClose,
}) => {
  const [expandedProviders, setExpandedProviders] = useState<Set<string>>(
    new Set(providers.length > 0 ? [providers[0].key] : [])
  );

  const toggleProvider = (key: string) => {
    setExpandedProviders(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  return (
    <div className="absolute top-full left-0 mt-1 w-72 bg-bg-secondary border border-border rounded-lg shadow-lg z-50 py-2 max-h-96 overflow-y-auto">
      {providers.map(provider => (
        <div key={provider.key}>
          <button
            onClick={() => toggleProvider(provider.key)}
            className="w-full flex items-center justify-between px-3 py-2 text-sm text-text hover:bg-bg transition-colors"
          >
            <span className="font-medium">{provider.name}</span>
            <span className="text-text-muted">
              {expandedProviders.has(provider.key) ? '▼' : '▶'}
            </span>
          </button>

          {expandedProviders.has(provider.key) && (
            <div className="pb-1">
              {provider.models.map((model: ModelItem) => (
                <button
                  key={model.key}
                  onClick={() => onSelect(provider.key, model.key, provider.key !== 'custom')}
                  className={`w-full text-left pl-6 pr-3 py-1.5 text-sm hover:bg-bg transition-colors ${
                    model.key === currentModel ? 'text-accent' : 'text-text-secondary'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span>{model.name}</span>
                    {model.key === currentModel && <span>✓</span>}
                  </div>
                  <div className="text-xs text-text-muted">
                    context: {model.context}, output: {model.output}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};
