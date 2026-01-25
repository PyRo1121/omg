export interface DocMetadata {
  title: string;
  slug: string;
  category?: string;
}

export interface DocContent {
  metadata: DocMetadata;
  content: string;
}

// Vite's import.meta.glob is great for this
const docs = import.meta.glob('../content/docs/**/*.md', { as: 'raw', eager: true });

export function getAllDocs(): DocMetadata[] {
  return Object.keys(docs).map((path) => {
    const slug = path
      .replace('../content/docs/', '')
      .replace('.md', '');
    
    // Simple title derivation from filename if no frontmatter parser is used
    const title = slug
      .split('/')
      .pop()!
      .replace(/-/g, ' ')
      .replace(/\b\w/g, (l) => l.toUpperCase());

    return { title, slug };
  });
}

export function getDocBySlug(slug: string): DocContent | null {
  const path = `../content/docs/${slug}.md`;
  const content = docs[path];

  if (!content) return null;

  const title = slug
    .split('/')
    .pop()!
    .replace(/-/g, ' ')
    .replace(/\b\w/g, (l) => l.toUpperCase());

  return {
    metadata: { title, slug },
    content: content as string,
  };
}
