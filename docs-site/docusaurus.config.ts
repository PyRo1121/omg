import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'OMG Documentation',
  tagline: 'The Fastest Unified Package Manager for Arch Linux + All Language Runtimes',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // Production URL
  url: 'https://pyro1121.com',
  baseUrl: '/docs/',

  organizationName: 'PyRo1121',
  projectName: 'omg',

  onBrokenLinks: 'warn',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  themes: [
    [
      '@easyops-cn/docusaurus-search-local',
      {
        hashed: true,
        language: ['en'],
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
        docsRouteBasePath: '/',
      },
    ],
  ],

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/', // Docs at root of /docs/
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/PyRo1121/omg/tree/main/docs-site/',
          // Versioning
          lastVersion: 'current',
          versions: {
            current: {
              label: 'Next',
              path: '',
            },
          },
        },
        blog: false, // Disable blog
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/omg-social-card.png',
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'OMG',
      logo: {
        alt: 'OMG Logo',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          type: 'docsVersionDropdown',
          position: 'right',
        },
        {
          href: 'https://pyro1121.com',
          label: 'Home',
          position: 'right',
        },
        {
          href: 'https://pyro1121.com/dashboard',
          label: 'Dashboard',
          position: 'right',
        },
        {
          href: 'https://github.com/PyRo1121/omg',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            { label: 'Getting Started', to: '/' },
            { label: 'CLI Reference', to: '/cli' },
            { label: 'Runtimes', to: '/runtimes' },
            { label: 'Security', to: '/security' },
          ],
        },
        {
          title: 'Resources',
          items: [
            { label: 'Architecture', to: '/architecture' },
            { label: 'Daemon', to: '/daemon' },
            { label: 'Troubleshooting', to: '/troubleshooting' },
          ],
        },
        {
          title: 'Community',
          items: [
            { label: 'GitHub', href: 'https://github.com/PyRo1121/omg' },
            { label: 'Issues', href: 'https://github.com/PyRo1121/omg/issues' },
            { label: 'Discussions', href: 'https://github.com/PyRo1121/omg/discussions' },
          ],
        },
        {
          title: 'Product',
          items: [
            { label: 'Home', href: 'https://pyro1121.com' },
            { label: 'Dashboard', href: 'https://pyro1121.com/dashboard' },
            { label: 'Pricing', href: 'https://pyro1121.com#pricing' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} OMG. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.vsDark,
      additionalLanguages: ['bash', 'toml', 'rust', 'json', 'typescript', 'python', 'go'],
      magicComments: [
        {
          className: 'theme-code-block-highlighted-line',
          line: 'highlight-next-line',
          block: { start: 'highlight-start', end: 'highlight-end' },
        },
      ],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
