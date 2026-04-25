// Sample Tailwind config used by Plumb's adapter tests.
// CommonJS shape — works without any TS loader installed.

module.exports = {
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
