import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'index',
    {
      type: 'category',
      label: 'Getting Started',
      collapsed: false,
      items: [
        'cli',
        'configuration',
        {
          type: 'category',
          label: 'Migration Guides',
          items: [
            'migration/from-yay',
            'migration/from-nvm',
            'migration/from-pyenv',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Core Concepts',
      items: [
        'architecture',
        'runtimes',
        'security',
        'workflows',
      ],
    },
    {
      type: 'category',
      label: 'Deep Dives',
      items: [
        'daemon',
        'cache',
        'ipc',
        'package-search',
        'cli-internals',
        'history',
        'tui',
      ],
    },
    'troubleshooting',
  ],
};

export default sidebars;
