/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        bg: 'var(--color-bg)',
        'bg-secondary': 'var(--color-bg-secondary)',
        'bg-overlay': 'var(--color-bg-overlay)',
        'bg-user-area': 'var(--color-bg-user-area)',
        'bg-ai-area': 'var(--color-bg-ai-area)',
        text: 'var(--color-text-primary)',
        'text-secondary': 'var(--color-text-secondary)',
        'text-muted': 'var(--color-text-muted)',
        border: 'var(--color-border)',
        brand: 'var(--color-brand)',
        'accent-hover': 'var(--color-accent-hover)',
        user: 'var(--color-user)',
        success: 'var(--color-success)',
        error: 'var(--color-error)',
        warning: 'var(--color-warning)',
        'selection-bg': 'var(--color-selection-bg)',
        'selection-fg': 'var(--color-selection-fg)',
        primary: '#24C8DB',
        'primary-hover': '#3DD4E6',
        tauri: {
          primary: '#24C8DB',
          secondary: '#C084FC',
          dark: '#020617',
          darker: '#01030C',
          card: '#0F172A',
          border: '#1E293B',
        },
      },
      fontFamily: {
        inter: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
    },
  },
  plugins: [],
};
