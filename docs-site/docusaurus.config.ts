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

  headTags: [
    {
      tagName: 'meta',
      attributes: {
        name: 'robots',
        content: 'index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        name: 'googlebot',
        content: 'index, follow',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        name: 'author',
        content: 'OMG Team',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        property: 'og:site_name',
        content: 'OMG Package Manager Documentation',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        property: 'og:type',
        content: 'website',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        property: 'og:locale',
        content: 'en_US',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        name: 'twitter:site',
        content: '@pyro1121',
      },
    },
    {
      tagName: 'meta',
      attributes: {
        name: 'twitter:card',
        content: 'summary_large_image',
      },
    },
    {
      tagName: 'link',
      attributes: {
        rel: 'preconnect',
        href: 'https://fonts.googleapis.com',
      },
    },
    {
      tagName: 'link',
      attributes: {
        rel: 'preconnect',
        href: 'https://fonts.gstatic.com',
        crossorigin: 'anonymous',
      },
    },
  ],

  themes: [],

  plugins: [
    require.resolve('docusaurus-lunr-search'),
    [
      '@docusaurus/plugin-pwa',
      {
        debug: false,
        offlineModeActivationStrategies: [
          'appInstalled',
          'standalone',
          'queryString',
        ],
        pwaHead: [
          {
            tagName: 'link',
            rel: 'icon',
            href: '/docs/img/logo.svg',
          },
          {
            tagName: 'link',
            rel: 'manifest',
            href: '/docs/manifest.json',
          },
          {
            tagName: 'meta',
            name: 'theme-color',
            content: '#6366f1',
          },
          {
            tagName: 'meta',
            name: 'apple-mobile-web-app-capable',
            content: 'yes',
          },
          {
            tagName: 'meta',
            name: 'apple-mobile-web-app-status-bar-style',
            content: 'black-translucent',
          },
          {
            tagName: 'link',
            rel: 'apple-touch-icon',
            href: '/docs/img/logo.svg',
          },
        ],
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
        sitemap: {
          lastmod: 'date',
          changefreq: 'weekly',
          priority: 0.7,
          ignorePatterns: ['/tags/**'],
          filename: 'sitemap.xml',
          createSitemapItems: async (params) => {
            const {defaultCreateSitemapItems, ...rest} = params;
            const items = await defaultCreateSitemapItems(rest);
            return items
              .filter((item) => !item.url.includes('/page/'))
              .map((item) => {
                if (item.url.includes('/quickstart') || item.url.includes('/cli') || item.url.endsWith('/docs/')) {
                  return { ...item, priority: 0.9 };
                }
                if (item.url.includes('/migration/') || item.url.includes('/runtimes') || item.url.includes('/packages')) {
                  return { ...item, priority: 0.85 };
                }
                if (item.url.includes('/faq') || item.url.includes('/troubleshooting')) {
                  return { ...item, priority: 0.8 };
                }
                return item;
              });
          },
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/omg-social-card.png',
    metadata: [
      {
        name: 'keywords',
        content: 'package manager, linux, arch linux, debian, ubuntu, nvm, pyenv, rustup, runtime manager, node version manager, python version manager, omg, unified package manager',
      },
      {
        name: 'description',
        content: 'Complete documentation for OMG, the fastest unified package manager for Linux. Learn to manage system packages and language runtimes with a single CLI.',
      },
    ],
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
