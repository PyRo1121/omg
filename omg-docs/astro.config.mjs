// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://pyro1121.com',
	base: '/docs',
	output: 'static',
	integrations: [
		starlight({
			title: 'OMG Docs',
			description: 'The fastest unified package manager for Arch Linux + all language runtimes. 22x faster than pacman.',
			logo: {
				light: './src/assets/logo-light.svg',
				dark: './src/assets/logo-dark.svg',
				replacesTitle: false,
			},
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/PyRo1121/omg' },
			],
			editLink: {
				baseUrl: 'https://github.com/PyRo1121/omg/edit/main/omg-docs/',
			},
			customCss: [
				'./src/styles/custom.css',
			],
			head: [
				{
					tag: 'meta',
					attrs: {
						property: 'og:image',
						content: 'https://docs.pyro1121.com/og-image.png',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:card',
						content: 'summary_large_image',
					},
				},
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'getting-started/introduction' },
						{ label: 'Quick Start', slug: 'getting-started/quickstart' },
					],
				},
				{
					label: 'CLI Reference',
					items: [
						{ label: 'All Commands', slug: 'cli/commands' },
					],
				},
			],
			tableOfContents: {
				minHeadingLevel: 2,
				maxHeadingLevel: 4,
			},
			lastUpdated: true,
			pagination: true,
		}),
	],
});
