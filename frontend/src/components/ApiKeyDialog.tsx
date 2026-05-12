import React, { useState } from 'react';
import { Dialog } from './ui/Dialog';
import { Button } from './ui/Button';

interface ApiKeyDialogProps {
  isOpen: boolean;
  provider: string;
  model: string;
  onSubmit: (apiKey: string) => void;
  onCancel: () => void;
}

export const ApiKeyDialog: React.FC<ApiKeyDialogProps> = ({
  isOpen,
  provider,
  model,
  onSubmit,
  onCancel,
}) => {
  const [apiKey, setApiKey] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(apiKey);
    setApiKey('');
  };

  return (
    <Dialog isOpen={isOpen} onClose={onCancel} title="API Key Required">
      <form onSubmit={handleSubmit} className="space-y-4">
        <p className="text-sm text-text-secondary">
          Switching to <span className="text-text font-medium">{model}</span> from{' '}
          <span className="text-text font-medium">{provider}</span>.
        </p>
        <p className="text-sm text-text-muted">
          Leave empty to use the configured API key.
        </p>

        <div>
          <label className="block text-sm font-medium text-text-secondary mb-1">
            API Key
          </label>
          <input
            type="password"
            value={apiKey}
            onChange={e => setApiKey(e.target.value)}
            placeholder="sk-..."
            className="w-full px-3 py-2 bg-bg border border-border rounded text-text placeholder-text-muted focus:outline-none focus:border-accent"
            autoFocus
          />
        </div>

        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel} type="button">
            Cancel
          </Button>
          <Button variant="primary" type="submit">
            Switch Model
          </Button>
        </div>
      </form>
    </Dialog>
  );
};
