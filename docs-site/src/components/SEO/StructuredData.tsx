import React from 'react';
import Head from '@docusaurus/Head';

interface StructuredDataProps {
  data: Record<string, unknown>;
}

export function StructuredData({ data }: StructuredDataProps) {
  const jsonLd = {
    '@context': 'https://schema.org',
    ...data,
  };

  return (
    <Head>
      <script type="application/ld+json">
        {JSON.stringify(jsonLd, null, 0)}
      </script>
    </Head>
  );
}

export function HowToSchema({
  name,
  description,
  totalTime,
  steps,
}: {
  name: string;
  description: string;
  totalTime?: string;
  steps: Array<{ name: string; text: string; url?: string }>;
}) {
  const data = {
    '@type': 'HowTo',
    name,
    description,
    ...(totalTime && { totalTime }),
    step: steps.map((step, index) => ({
      '@type': 'HowToStep',
      position: index + 1,
      name: step.name,
      text: step.text,
      ...(step.url && { url: step.url }),
    })),
  };

  return <StructuredData data={data} />;
}

export function FAQSchema({
  questions,
}: {
  questions: Array<{ question: string; answer: string }>;
}) {
  const data = {
    '@type': 'FAQPage',
    mainEntity: questions.map((q) => ({
      '@type': 'Question',
      name: q.question,
      acceptedAnswer: {
        '@type': 'Answer',
        text: q.answer,
      },
    })),
  };

  return <StructuredData data={data} />;
}

export function ArticleSchema({
  headline,
  description,
  datePublished,
  dateModified,
  author,
  url,
}: {
  headline: string;
  description: string;
  datePublished?: string;
  dateModified?: string;
  author?: string;
  url?: string;
}) {
  const data = {
    '@type': 'TechArticle',
    headline,
    description,
    author: {
      '@type': 'Organization',
      name: author || 'OMG Team',
      url: 'https://pyro1121.com',
    },
    publisher: {
      '@type': 'Organization',
      name: 'OMG Package Manager',
      url: 'https://pyro1121.com',
      logo: {
        '@type': 'ImageObject',
        url: 'https://pyro1121.com/favicon.png',
      },
    },
    ...(datePublished && { datePublished }),
    ...(dateModified && { dateModified }),
    ...(url && { url }),
    mainEntityOfPage: {
      '@type': 'WebPage',
      '@id': url || 'https://pyro1121.com/docs/',
    },
  };

  return <StructuredData data={data} />;
}

export function BreadcrumbSchema({
  items,
}: {
  items: Array<{ name: string; url: string }>;
}) {
  const data = {
    '@type': 'BreadcrumbList',
    itemListElement: items.map((item, index) => ({
      '@type': 'ListItem',
      position: index + 1,
      name: item.name,
      item: item.url,
    })),
  };

  return <StructuredData data={data} />;
}

export function SoftwareApplicationSchema() {
  const data = {
    '@type': 'SoftwareApplication',
    name: 'OMG Package Manager',
    alternateName: ['OMG', 'omg-cli'],
    description:
      'The fastest unified package manager for Linux. Manage system packages and language runtimes with a single CLI.',
    operatingSystem: ['Arch Linux', 'Debian', 'Ubuntu', 'Linux'],
    applicationCategory: 'DeveloperApplication',
    applicationSubCategory: 'Package Manager',
    offers: {
      '@type': 'Offer',
      price: '0',
      priceCurrency: 'USD',
    },
    author: {
      '@type': 'Organization',
      name: 'OMG Team',
      url: 'https://pyro1121.com',
    },
    url: 'https://pyro1121.com/',
  };

  return <StructuredData data={data} />;
}
