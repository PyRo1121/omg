import { Component, createResource, Show } from 'solid-js';
import { createHighlighterCore, HighlighterCore } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';

let highlighterPromise: Promise<HighlighterCore> | null = null;

const getSharedHighlighter = () => {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      themes: [import('@shikijs/themes/github-dark')],
      langs: [
        import('@shikijs/langs/javascript'),
        import('@shikijs/langs/typescript'),
        import('@shikijs/langs/tsx'),
        import('@shikijs/langs/bash'),
        import('@shikijs/langs/json'),
        import('@shikijs/langs/yaml'),
        import('@shikijs/langs/markdown'),
        import('@shikijs/langs/rust'),
      ],
      engine: createJavaScriptRegexEngine(),
    });
  }
  return highlighterPromise;
};

interface CodeBlockProps {
  children?: any;
  className?: string;
  inline?: boolean;
}

const CodeBlock: Component<CodeBlockProps> = props => {
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
        theme: 'github-dark',
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
