// Sample Tailwind TypeScript config for Plumb's adapter round-trip
// test. The Rust integration test only runs this when a TS loader
// (`tsx`, `ts-node`, or `esbuild-register`) is installed in the host
// project — otherwise the loader emits TS_LOADER_MISSING and the test
// skips.

import type { Config } from 'tailwindcss';

const config: Config = {
  content: [],
  theme: {
    colors: {
      white: '#ffffff',
      black: '#000000',
      red: {
        500: '#ef4444',
        600: '#dc2626',
      },
    },
    spacing: {
      0: '0',
      1: '0.25rem',
      2: '0.5rem',
      4: '1rem',
      8: '2rem',
    },
    fontSize: {
      sm: ['0.875rem', { lineHeight: '1.25rem' }],
      base: ['1rem', { lineHeight: '1.5rem' }],
      lg: ['1.125rem', { lineHeight: '1.75rem' }],
    },
    fontWeight: {
      normal: '400',
      medium: '500',
      bold: '700',
    },
    fontFamily: {
      sans: ['Inter', 'ui-sans-serif', 'system-ui'],
      mono: ['JetBrains Mono', 'monospace'],
    },
    borderRadius: {
      none: '0',
      sm: '0.125rem',
      DEFAULT: '0.25rem',
      md: '0.375rem',
      lg: '0.5rem',
    },
  },
};

export default config;
