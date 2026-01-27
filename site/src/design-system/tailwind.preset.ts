import type { Config } from 'tailwindcss';

export const omgPreset: Partial<Config> = {
  theme: {
    extend: {
      fontFamily: {
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
        body: ['Plus Jakarta Sans', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'monospace'],
      },

      colors: {
        void: {
          950: '#030304',
          900: '#07070a',
          850: '#0a0a0e',
          800: '#0d0d12',
          750: '#101016',
          700: '#14141b',
          650: '#181820',
          600: '#1c1c26',
          500: '#24242f',
          400: '#2e2e3a',
          300: '#3d3d4a',
          200: '#52525e',
          100: '#71717a',
        },
        nebula: {
          950: '#0f0f14',
          900: '#18181f',
          800: '#27272f',
          700: '#3f3f47',
          600: '#52525a',
          500: '#71717a',
          400: '#a1a1aa',
          300: '#d4d4dc',
          200: '#e4e4ec',
          100: '#f4f4f8',
          50: '#fafafc',
        },
        plasma: {
          950: '#0c1929',
          900: '#122a4a',
          800: '#1a3a66',
          700: '#234e8a',
          600: '#2e64ad',
          500: '#3b7dd1',
          400: '#5a9ae8',
          300: '#85b8f4',
          200: '#b3d4fa',
          100: '#dceafd',
          50: '#f0f6fe',
        },
        electric: {
          950: '#042f2e',
          900: '#064e4b',
          800: '#0d7377',
          700: '#0e9494',
          600: '#14b8b8',
          500: '#22d3d3',
          400: '#2ee8e8',
          300: '#5ff5f5',
          200: '#99f6f6',
          100: '#ccfbfb',
          50: '#ecfefe',
        },
        photon: {
          950: '#1e1028',
          900: '#2e1a42',
          800: '#452766',
          700: '#5e3488',
          600: '#7c3aad',
          500: '#9d4edd',
          400: '#b06de8',
          300: '#c89af0',
          200: '#e0c3f7',
          100: '#f0e1fb',
          50: '#f9f1fd',
        },
        aurora: {
          950: '#022c22',
          900: '#064e3b',
          800: '#065f46',
          700: '#047857',
          600: '#059669',
          500: '#10b981',
          400: '#34d399',
          300: '#6ee7b7',
          200: '#a7f3d0',
          100: '#d1fae5',
          50: '#ecfdf5',
        },
        solar: {
          950: '#2a1f00',
          900: '#453a00',
          800: '#6b5900',
          700: '#8a7500',
          600: '#b89500',
          500: '#f59e0b',
          400: '#fbbf24',
          300: '#fcd34d',
          200: '#fde68a',
          100: '#fef3c7',
          50: '#fffbeb',
        },
        flare: {
          950: '#2c0a0e',
          900: '#4c1016',
          800: '#7f1d1d',
          700: '#991b1b',
          600: '#dc2626',
          500: '#ef4444',
          400: '#f87171',
          300: '#fca5a5',
          200: '#fecaca',
          100: '#fee2e2',
          50: '#fef2f2',
        },
      },

      spacing: {
        '0.5': '0.125rem',
        '1.5': '0.375rem',
        '2.5': '0.625rem',
        '3.5': '0.875rem',
        '4.5': '1.125rem',
        '13': '3.25rem',
        '15': '3.75rem',
        '18': '4.5rem',
        '22': '5.5rem',
      },

      borderRadius: {
        '4xl': '2rem',
        '5xl': '2.5rem',
      },

      fontSize: {
        '2xs': ['0.625rem', { lineHeight: '0.875rem' }],
      },

      boxShadow: {
        'glow-sm': '0 0 10px var(--color-indigo-500, #6366f1)',
        'glow-md': '0 0 20px var(--color-indigo-500, #6366f1), 0 0 40px rgba(99, 102, 241, 0.3)',
        'glow-lg': '0 0 30px var(--color-indigo-500, #6366f1), 0 0 60px rgba(99, 102, 241, 0.4)',
        'glow-electric': '0 0 20px var(--color-electric-500, #22d3d3), 0 0 40px rgba(34, 211, 211, 0.3)',
        'glow-aurora': '0 0 20px var(--color-aurora-500, #10b981), 0 0 40px rgba(16, 185, 129, 0.3)',
        'glow-flare': '0 0 20px var(--color-flare-500, #ef4444), 0 0 40px rgba(239, 68, 68, 0.3)',
        'card': '0 0 0 1px rgba(255,255,255,0.05), 0 4px 6px -1px rgba(0,0,0,0.4), 0 2px 4px -2px rgba(0,0,0,0.4)',
        'card-hover': '0 0 0 1px rgba(255,255,255,0.08), 0 10px 15px -3px rgba(0,0,0,0.5), 0 4px 6px -4px rgba(0,0,0,0.5), 0 0 30px -10px rgba(99,102,241,0.2)',
        '3xl': '0 35px 60px -15px rgba(0, 0, 0, 0.7)',
      },

      animation: {
        'pulse-glow': 'pulse-glow 2s ease-in-out infinite',
        'pulse-slow': 'pulse-slow 4s ease-in-out infinite',
        'float': 'float 3s ease-in-out infinite',
        'shimmer': 'shimmer 2s linear infinite',
        'score-fill': 'score-fill 1s ease-out forwards',
        'fade-up': 'fade-up 0.5s ease-out forwards',
        'slide-in-right': 'slide-in-right 0.3s ease-out forwards',
        'slide-in-bottom': 'slide-in-bottom 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards',
        'scale-in': 'scale-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards',
        'spin-slow': 'spin 3s linear infinite',
        'data-pulse': 'data-pulse 1.5s ease-in-out infinite',
        'stream-in': 'stream-in 0.5s ease-out forwards',
        'gauge-fill': 'gauge-fill 1.2s cubic-bezier(0.34, 1.56, 0.64, 1) forwards',
        'ring-expand': 'ring-expand 1s ease-out forwards',
      },

      keyframes: {
        'pulse-glow': {
          '0%, 100%': { boxShadow: '0 0 20px rgba(99, 102, 241, 0.3)' },
          '50%': { boxShadow: '0 0 40px rgba(99, 102, 241, 0.6)' },
        },
        'pulse-slow': {
          '0%, 100%': { opacity: '0.3', transform: 'scale(1)' },
          '50%': { opacity: '0.6', transform: 'scale(1.1)' },
        },
        float: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-10px)' },
        },
        shimmer: {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        'score-fill': {
          '0%': { strokeDashoffset: '100' },
          '100%': { strokeDashoffset: 'var(--score-offset, 0)' },
        },
        'fade-up': {
          '0%': { opacity: '0', transform: 'translateY(20px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'slide-in-right': {
          '0%': { opacity: '0', transform: 'translateX(20px)' },
          '100%': { opacity: '1', transform: 'translateX(0)' },
        },
        'slide-in-bottom': {
          '0%': { opacity: '0', transform: 'translateY(10px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'scale-in': {
          '0%': { opacity: '0', transform: 'scale(0.95)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
        'data-pulse': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.5' },
        },
        'stream-in': {
          '0%': { opacity: '0', transform: 'translateX(-10px) scale(0.95)' },
          '100%': { opacity: '1', transform: 'translateX(0) scale(1)' },
        },
        'gauge-fill': {
          '0%': { strokeDashoffset: 'var(--gauge-circumference, 283)' },
          '100%': { strokeDashoffset: 'var(--gauge-offset, 0)' },
        },
        'ring-expand': {
          '0%': { transform: 'scale(0.8)', opacity: '1' },
          '100%': { transform: 'scale(2)', opacity: '0' },
        },
      },

      transitionTimingFunction: {
        'spring': 'cubic-bezier(0.34, 1.56, 0.64, 1)',
        'bounce': 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
        'smooth': 'cubic-bezier(0.23, 1, 0.32, 1)',
        'swift': 'cubic-bezier(0.16, 1, 0.3, 1)',
      },

      backdropBlur: {
        xs: '2px',
      },

      backgroundImage: {
        'gradient-radial': 'radial-gradient(circle, var(--tw-gradient-stops))',
        'gradient-conic': 'conic-gradient(from 180deg at 50% 50%, var(--tw-gradient-stops))',
        'mesh-gradient': `
          radial-gradient(at 40% 20%, rgba(99, 102, 241, 0.1) 0px, transparent 50%),
          radial-gradient(at 80% 0%, rgba(34, 211, 238, 0.08) 0px, transparent 50%),
          radial-gradient(at 0% 50%, rgba(168, 85, 247, 0.08) 0px, transparent 50%),
          radial-gradient(at 80% 50%, rgba(16, 185, 129, 0.05) 0px, transparent 50%),
          radial-gradient(at 0% 100%, rgba(99, 102, 241, 0.08) 0px, transparent 50%)
        `,
        'noise': "url(\"data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E\")",
      },
    },
  },
  plugins: [],
};

export default omgPreset;
