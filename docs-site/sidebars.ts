import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * PROGRESSIVE DISCLOSURE SIDEBAR - 2 LEVELS MAX
 * Following best practices from Stripe, Vercel, and Nielsen Norman Group
 * ═══════════════════════════════════════════════════════════════════════════
 */

const sidebars: SidebarsConfig = {
  docsSidebar: [
    // ⚡ QUICK START - Always visible, never collapsed
    'quickstart',

    // 📚 GUIDES - Common tasks users want to accomplish
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      items: [
        'packages',        // Managing system packages
        'runtimes',        // Managing language runtimes
        'shell-integration', // Shell integration & hooks
        'task-runner',     // Running tasks with correct versions
        'team',            // Team workflows & lock files
        'security',        // Security features & SBOM
        'containers',      // Docker & OCI images
      ],
    },

    // 🔄 MIGRATION - For users switching from other tools
    {
      type: 'category',
      label: 'Migration',
      collapsed: true,
      items: [
        'migration/from-yay',    // From pacman/yay
        'migration/from-nvm',    // From nvm
        'migration/from-pyenv',  // From pyenv
      ],
    },

    // 📖 REFERENCE - Collapsed by default (progressive disclosure)
    {
      type: 'category',
      label: 'Reference',
      collapsed: true,
      items: [
        'cli',             // CLI command reference
        'configuration',   // Config file reference
        'api-reference',   // API reference
        'workflows',       // Common workflows
        'tui',             // Terminal UI
        'history',         // Command history
        'troubleshooting', // Troubleshooting guide
        'faq',             // FAQ
      ],
    },

    // 🔧 ADVANCED - Collapsed by default (only for power users)
    {
      type: 'category',
      label: 'Advanced',
      collapsed: true,
      items: [
        'white-paper',           // Technical white paper
        'architecture',          // System architecture
        'daemon',                // Daemon internals
        'cache',                 // Cache system
        'ipc',                   // IPC protocol
        'package-search',        // Package search internals
        'cli-internals',         // CLI internals
        'fast-status-deep-dive', // Performance deep dive
        'search-performance-deep-dive', // Search performance
        'shell-hook-deep-dive',  // Shell hook internals
      ],
    },

    // 📝 META - Always at the bottom
    {
      type: 'category',
      label: 'About',
      collapsed: true,
      items: [
        'index',         // Introduction
        'changelog',     // Changelog
      ],
    },
  ],
};

export default sidebars;
