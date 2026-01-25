import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'index',
    'white-paper',
    {
      type: 'category',
      label: 'Getting Started',
      collapsed: false,
      items: [
        'quickstart',
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
      label: 'Core Features',
      items: [
        'packages',
        'runtimes',
        'shell-integration',
        'task-runner',
      ],
    },
    {
      type: 'category',
      label: 'Advanced Features',
      items: [
        'security',
        'team',
        'containers',
        'tui',
        'history',
      ],
    },
    {
      type: 'category',
      label: 'Architecture & Internals',
      items: [
        'architecture',
        'daemon',
        'cache',
        'ipc',
        'package-search',
        'cli-internals',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'api-reference',
        'workflows',
        'troubleshooting',
        'faq',
        'changelog',
      ],
    },
  ],
};

export default sidebars;
