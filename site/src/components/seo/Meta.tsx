import { Component, createMemo, For, Show } from 'solid-js';
import { Title, Meta as SolidMeta, Link } from '@solidjs/meta';

// ============================================================================
// Types
// ============================================================================

interface SeoMetaProps {
  /** Page title - will be appended with site name */
  title: string;
  /** Meta description for search results */
  description: string;
  /** Canonical URL for this page */
  canonical?: string;
  /** Type of page for Open Graph */
  type?: 'website' | 'article' | 'product';
  /** Open Graph image URL */
  image?: string;
  /** Image alt text */
  imageAlt?: string;
  /** Article publish date (ISO format) */
  publishedTime?: string;
  /** Article modified date (ISO format) */
  modifiedTime?: string;
  /** Author name for articles */
  author?: string;
  /** Keywords for the page */
  keywords?: string[];
  /** Disable indexing for this page */
  noindex?: boolean;
  /** Custom JSON-LD structured data to include */
  structuredData?: StructuredData[];
  /** Breadcrumb items for BreadcrumbList schema */
  breadcrumbs?: BreadcrumbItem[];
}

interface BreadcrumbItem {
  name: string;
  url: string;
}

type StructuredData = OrganizationSchema | SoftwareApplicationSchema | BreadcrumbListSchema | WebPageSchema;

interface OrganizationSchema {
  '@type': 'Organization';
  name: string;
  url: string;
  logo?: string;
  sameAs?: string[];
  description?: string;
}

interface SoftwareApplicationSchema {
  '@type': 'SoftwareApplication';
  name: string;
  description: string;
  applicationCategory: string;
  operatingSystem: string;
  offers?: {
    '@type': 'Offer';
    price: string;
    priceCurrency: string;
  };
  aggregateRating?: {
    '@type': 'AggregateRating';
    ratingValue: string;
    ratingCount: string;
  };
}

interface BreadcrumbListSchema {
  '@type': 'BreadcrumbList';
  itemListElement: {
    '@type': 'ListItem';
    position: number;
    name: string;
    item: string;
  }[];
}

interface WebPageSchema {
  '@type': 'WebPage';
  name: string;
  description: string;
  url: string;
}

// ============================================================================
// Constants
// ============================================================================

const SITE_NAME = 'OMG Package Manager';
const SITE_URL = 'https://pyro1121.com';
const DEFAULT_IMAGE = `${SITE_URL}/og/omg-og.png`;
const DEFAULT_IMAGE_ALT = 'OMG - The Fastest Linux Package Manager';
const TWITTER_HANDLE = '@omgpkg';

// Default Organization schema
const ORGANIZATION_SCHEMA: OrganizationSchema = {
  '@type': 'Organization',
  name: 'OMG Package Manager',
  url: SITE_URL,
  logo: `${SITE_URL}/logo-globe.png`,
  sameAs: ['https://github.com/PyRo1121/omg', 'https://twitter.com/omgpkg'],
  description:
    'OMG is the fastest unified package manager for Linux, combining system packages and language runtimes in a single CLI.',
};

// Default SoftwareApplication schema
const SOFTWARE_SCHEMA: SoftwareApplicationSchema = {
  '@type': 'SoftwareApplication',
  name: 'OMG',
  description:
    'Unified package manager and runtime manager for Linux. 22x faster than pacman with native Node.js, Python, Go, Rust, Ruby, Java, and Bun support.',
  applicationCategory: 'DeveloperApplication',
  operatingSystem: 'Linux',
  offers: {
    '@type': 'Offer',
    price: '0',
    priceCurrency: 'USD',
  },
};

// ============================================================================
// Component
// ============================================================================

const SeoMeta: Component<SeoMetaProps> = (props) => {
  // Build full title with site name
  const fullTitle = createMemo(() => {
    if (props.title === SITE_NAME) {
      return props.title;
    }
    return `${props.title} | ${SITE_NAME}`;
  });

  // Canonical URL
  const canonicalUrl = createMemo(() => {
    if (props.canonical) {
      return props.canonical.startsWith('http') ? props.canonical : `${SITE_URL}${props.canonical}`;
    }
    return undefined;
  });

  // Image URL
  const imageUrl = createMemo(() => {
    const img = props.image || DEFAULT_IMAGE;
    return img.startsWith('http') ? img : `${SITE_URL}${img}`;
  });

  // Build BreadcrumbList schema from props
  const breadcrumbSchema = createMemo((): BreadcrumbListSchema | null => {
    if (!props.breadcrumbs || props.breadcrumbs.length === 0) return null;

    return {
      '@type': 'BreadcrumbList',
      itemListElement: props.breadcrumbs.map((item, index) => ({
        '@type': 'ListItem',
        position: index + 1,
        name: item.name,
        item: item.url.startsWith('http') ? item.url : `${SITE_URL}${item.url}`,
      })),
    };
  });

  // Combine all structured data
  const allStructuredData = createMemo(() => {
    const data: StructuredData[] = [ORGANIZATION_SCHEMA, SOFTWARE_SCHEMA];

    // Add breadcrumbs if provided
    const breadcrumbs = breadcrumbSchema();
    if (breadcrumbs) {
      data.push(breadcrumbs);
    }

    // Add custom structured data
    if (props.structuredData) {
      data.push(...props.structuredData);
    }

    return data;
  });

  // Generate JSON-LD script content
  const jsonLd = createMemo(() => {
    const graph = {
      '@context': 'https://schema.org',
      '@graph': allStructuredData(),
    };
    return JSON.stringify(graph);
  });

  return (
    <>
      {/* Primary Meta Tags */}
      <Title>{fullTitle()}</Title>
      <SolidMeta name="description" content={props.description} />
      <Show when={props.keywords && props.keywords.length > 0}>
        <SolidMeta name="keywords" content={props.keywords!.join(', ')} />
      </Show>
      <Show when={props.author}>
        <SolidMeta name="author" content={props.author} />
      </Show>

      {/* Robots */}
      <Show when={props.noindex}>
        <SolidMeta name="robots" content="noindex, nofollow" />
      </Show>
      <Show when={!props.noindex}>
        <SolidMeta name="robots" content="index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1" />
      </Show>

      {/* Canonical URL */}
      <Show when={canonicalUrl()}>
        <Link rel="canonical" href={canonicalUrl()!} />
      </Show>

      {/* Open Graph */}
      <SolidMeta property="og:type" content={props.type || 'website'} />
      <SolidMeta property="og:site_name" content={SITE_NAME} />
      <SolidMeta property="og:title" content={fullTitle()} />
      <SolidMeta property="og:description" content={props.description} />
      <SolidMeta property="og:image" content={imageUrl()} />
      <SolidMeta property="og:image:alt" content={props.imageAlt || DEFAULT_IMAGE_ALT} />
      <SolidMeta property="og:image:width" content="1200" />
      <SolidMeta property="og:image:height" content="630" />
      <Show when={canonicalUrl()}>
        <SolidMeta property="og:url" content={canonicalUrl()!} />
      </Show>
      <SolidMeta property="og:locale" content="en_US" />

      {/* Article-specific OG tags */}
      <Show when={props.type === 'article'}>
        <Show when={props.publishedTime}>
          <SolidMeta property="article:published_time" content={props.publishedTime} />
        </Show>
        <Show when={props.modifiedTime}>
          <SolidMeta property="article:modified_time" content={props.modifiedTime} />
        </Show>
        <Show when={props.author}>
          <SolidMeta property="article:author" content={props.author} />
        </Show>
      </Show>

      {/* Twitter Card */}
      <SolidMeta name="twitter:card" content="summary_large_image" />
      <SolidMeta name="twitter:site" content={TWITTER_HANDLE} />
      <SolidMeta name="twitter:creator" content={TWITTER_HANDLE} />
      <SolidMeta name="twitter:title" content={fullTitle()} />
      <SolidMeta name="twitter:description" content={props.description} />
      <SolidMeta name="twitter:image" content={imageUrl()} />
      <SolidMeta name="twitter:image:alt" content={props.imageAlt || DEFAULT_IMAGE_ALT} />

      {/* JSON-LD Structured Data */}
      <script type="application/ld+json" innerHTML={jsonLd()} />
    </>
  );
};

export default SeoMeta;

// ============================================================================
// Helper Functions for Common Schemas
// ============================================================================

/**
 * Create a WebPage schema for a specific page
 */
export function createWebPageSchema(name: string, description: string, url: string): WebPageSchema {
  return {
    '@type': 'WebPage',
    name,
    description,
    url: url.startsWith('http') ? url : `${SITE_URL}${url}`,
  };
}

/**
 * Create breadcrumb items from a path
 */
export function createBreadcrumbsFromPath(
  path: string,
  customLabels?: Record<string, string>
): BreadcrumbItem[] {
  const breadcrumbs: BreadcrumbItem[] = [{ name: 'Home', url: '/' }];

  if (!path || path === '/') {
    return breadcrumbs;
  }

  const segments = path.split('/').filter(Boolean);
  let currentPath = '';

  for (const segment of segments) {
    currentPath += `/${segment}`;
    const label =
      customLabels?.[segment] ||
      segment
        .split('-')
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
    breadcrumbs.push({ name: label, url: currentPath });
  }

  return breadcrumbs;
}

// Export types for external use
export type { SeoMetaProps, BreadcrumbItem, StructuredData };
