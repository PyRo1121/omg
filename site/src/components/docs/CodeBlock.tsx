import { Component, createResource, Show } from 'solid-js';
import { createHighlighter, Highlighter } from 'shiki';

let highlighterPromise: Promise<Highlighter> | null = null;

const getSharedHighlighter = () => {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ['github-dark'],
      langs: ['javascript', 'typescript', 'tsx', 'bash', 'json', 'yaml', 'markdown', 'rust'],
    });
  }
  return highlighterPromise;
};

interface CodeBlockProps {
  children?: any;
  className?: string;
  inline?: boolean;
}

const CodeBlock: Component<CodeBlockProps> = (props) => {
  // Extract language from className (e.g., "language-js")
  const lang = () => {
    const match = /language-(\w+)/.exec(props.className || '');
    return match ? match[1] : '';
  };

  const code = () => {
    if (Array.isArray(props.children)) {
      return props.children.join('').trim();
    }
    return String(props.children || '').trim();
  };

  const [highlighter] = createResource(getSharedHighlighter);

  const highlightedHtml = () => {
    const h = highlighter();
    if (!h) return '';
    
    try {
      return h.codeToHtml(code(), {
        lang: lang() || 'text',
        theme: 'github-dark'
      });
    } catch (e) {
      console.error('Shiki highlighting failed:', e);
      return `<code>${code()}</code>`;
    }
  };

  return (
    <Show
      when={!props.inline && highlighter()}
      fallback={<code class={props.className}>{props.children}</code>}
    >
      <div class="shiki-wrapper" innerHTML={highlightedHtml()} />
    </Show>
  );
};

export default CodeBlock;
