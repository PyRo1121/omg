import React, { useEffect } from 'react';
import { useLocation } from '@docusaurus/router';
import Head from '@docusaurus/Head';
import { initAnalytics } from '../lib/analytics';

function GlobalSEO() {
  return (
    <Head>
      <script type="application/ld+json">
        {JSON.stringify({
          '@context': 'https://schema.org',
          '@type': 'Organization',
          name: 'OMG Package Manager',
          alternateName: ['OMG', 'OhMyOpenCode'],
          url: 'https://pyro1121.com/',
          logo: {
            '@type': 'ImageObject',
            url: 'https://pyro1121.com/docs/img/logo.svg',
            width: 512,
            height: 512,
          },
          description:
            'The fastest unified package manager for Linux. One CLI for system packages and language runtimes.',
          sameAs: ['https://github.com/PyRo1121/omg'],
        })}
      </script>
    </Head>
  );
}

function QuickstartSEO() {
  return (
    <Head>
      <script type="application/ld+json">
        {JSON.stringify({
          '@context': 'https://schema.org',
          '@type': 'HowTo',
          name: 'How to Install and Set Up OMG Package Manager',
          description:
            'Complete guide to installing OMG, the fastest unified package manager for Linux. Set up shell integration and start managing packages and runtimes in 5 minutes.',
          totalTime: 'PT5M',
          estimatedCost: {
            '@type': 'MonetaryAmount',
            currency: 'USD',
            value: '0',
          },
          tool: [
            { '@type': 'HowToTool', name: 'Terminal (bash, zsh, or fish)' },
            { '@type': 'HowToTool', name: 'curl' },
          ],
          step: [
            {
              '@type': 'HowToStep',
              position: 1,
              name: 'Install OMG',
              text: 'Run the install script: curl -fsSL https://pyro1121.com/install.sh | bash',
              url: 'https://pyro1121.com/docs/quickstart#install-omg',
            },
            {
              '@type': 'HowToStep',
              position: 2,
              name: 'Set up shell integration',
              text: 'Add the shell hook to your config. For zsh: eval "$(omg hook zsh)"',
              url: 'https://pyro1121.com/docs/quickstart#set-up-your-shell',
            },
            {
              '@type': 'HowToStep',
              position: 3,
              name: 'Verify installation',
              text: 'Run omg --version and omg doctor to verify everything works',
              url: 'https://pyro1121.com/docs/quickstart#verify-it-works',
            },
            {
              '@type': 'HowToStep',
              position: 4,
              name: 'Search and install packages',
              text: 'Try omg search neovim and omg install neovim',
              url: 'https://pyro1121.com/docs/quickstart#your-first-60-seconds-with-omg',
            },
            {
              '@type': 'HowToStep',
              position: 5,
              name: 'Switch runtime versions',
              text: 'Use omg use node 20 to install and switch to Node.js 20',
              url: 'https://pyro1121.com/docs/quickstart#switch-nodejs-versions',
            },
          ],
        })}
      </script>
    </Head>
  );
}

function FAQSEO() {
  return (
    <Head>
      <script type="application/ld+json">
        {JSON.stringify({
          '@context': 'https://schema.org',
          '@type': 'FAQPage',
          mainEntity: [
            {
              '@type': 'Question',
              name: 'What is OMG Package Manager?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'OMG is a unified package and runtime manager for Linux that combines system package management (like pacman, apt) with language runtime management (like nvm, pyenv) into a single CLI. Written in pure Rust, it achieves 22x faster performance than pacman.',
              },
            },
            {
              '@type': 'Question',
              name: 'Which Linux distributions does OMG support?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'OMG supports Arch Linux (and derivatives like Manjaro, EndeavourOS), Debian, and Ubuntu. It uses native package manager backends - libalpm for Arch and rust-apt for Debian/Ubuntu.',
              },
            },
            {
              '@type': 'Question',
              name: 'What runtimes can OMG manage?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'OMG has native support for Node.js, Python, Go, Rust, Ruby, Java, and Bun. Through built-in mise integration, it supports 100+ additional runtimes including Deno, Elixir, Zig, PHP, and more.',
              },
            },
            {
              '@type': 'Question',
              name: 'How fast is OMG compared to other package managers?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'OMG achieves 6ms average search time, making it 22x faster than pacman (133ms) and 59-483x faster than apt-cache on Debian/Ubuntu. Runtime version switching takes just 1.8ms compared to 100-200ms for nvm/pyenv.',
              },
            },
            {
              '@type': 'Question',
              name: 'Is OMG free to use?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Yes! OMG core features are free forever under AGPL-3.0, including package management, runtime switching, and basic security. Pro ($9/mo) and Team ($29/mo) tiers add enterprise features like SBOM generation, vulnerability scanning, and team sync.',
              },
            },
            {
              '@type': 'Question',
              name: 'Can OMG replace nvm, pyenv, and rustup?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Yes. OMG provides native runtime management for Node.js, Python, Rust, Go, Ruby, Java, and Bun. It detects .nvmrc, .python-version, rust-toolchain.toml, and other version files automatically.',
              },
            },
            {
              '@type': 'Question',
              name: 'Does OMG work with the AUR?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Yes. OMG has full AUR support with intelligent package source detection. It automatically determines whether a package is in official repos or AUR and handles installation appropriately.',
              },
            },
            {
              '@type': 'Question',
              name: 'How do I install OMG?',
              acceptedAnswer: {
                '@type': 'Answer',
                text: 'Run: curl -fsSL https://pyro1121.com/install.sh | bash. Then add the shell hook to your config: eval "$(omg hook zsh)" for zsh users.',
              },
            },
          ],
        })}
      </script>
    </Head>
  );
}

function BreadcrumbSEO() {
  const location = useLocation();
  const path = location.pathname;

  if (!path.startsWith('/docs') || path === '/docs/' || path === '/docs') {
    return null;
  }

  const pathSegments = path.replace('/docs/', '').replace('/docs', '').split('/').filter(Boolean);
  const breadcrumbs = [
    { name: 'Home', url: 'https://pyro1121.com/' },
    { name: 'Documentation', url: 'https://pyro1121.com/docs/' },
  ];

  let currentPath = '/docs';
  for (const segment of pathSegments) {
    currentPath += `/${segment}`;
    const name = segment
      .split('-')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
    breadcrumbs.push({ name, url: `https://pyro1121.com${currentPath}` });
  }

  return (
    <Head>
      <script type="application/ld+json">
        {JSON.stringify({
          '@context': 'https://schema.org',
          '@type': 'BreadcrumbList',
          itemListElement: breadcrumbs.map((item, index) => ({
            '@type': 'ListItem',
            position: index + 1,
            name: item.name,
            item: item.url,
          })),
        })}
      </script>
    </Head>
  );
}

function PageSpecificSEO() {
  const location = useLocation();
  const path = location.pathname;

  if (path === '/docs/quickstart' || path === '/docs/quickstart/') {
    return <QuickstartSEO />;
  }

  if (path === '/docs/faq' || path === '/docs/faq/') {
    return <FAQSEO />;
  }

  return null;
}

export default function Root({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    const analytics = initAnalytics();
    return () => {
      analytics?.destroy();
    };
  }, []);

  return (
    <>
      <GlobalSEO />
      <BreadcrumbSEO />
      <PageSpecificSEO />
      {children}
    </>
  );
}
