import { Component, createMemo, For } from 'solid-js';
import { useParams, A } from '@solidjs/router';
import { SolidMarkdown } from 'solid-markdown';
import { getAllDocs, getDocBySlug } from '../lib/docs';
import GlassCard from '../components/ui/GlassCard';
import Header from '../components/Header';

const DocsPage: Component = () => {
  const params = useParams();
  const allDocs = getAllDocs();

  const currentDoc = createMemo(() => {
    const slug = params.slug || 'index';
    return getDocBySlug(slug);
  });

  return (
    <div class="min-h-screen flex flex-col">
      <Header />
      
      <main class="flex-grow pt-24 pb-20 px-6 max-w-7xl mx-auto w-full grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-10">
        {/* Sidebar */}
        <aside class="hidden lg:block sticky top-24 self-start">
          <GlassCard class="p-6 max-h-[calc(100vh-120px)] overflow-y-auto">
            <h3 class="text-white font-bold mb-4 px-2">Documentation</h3>
            <nav class="flex flex-col gap-1">
              <For each={allDocs}>
                {(doc) => (
                  <A
                    href={`/docs/${doc.slug}`}
                    class="px-3 py-2 rounded-lg text-sm transition-colors"
                    activeClass="bg-indigo-500/20 text-indigo-300 border border-indigo-500/30"
                    inactiveClass="text-slate-400 hover:text-white hover:bg-white/5"
                  >
                    {doc.title}
                  </A>
                )}
              </For>
            </nav>
          </GlassCard>
        </aside>

        {/* Content */}
        <div class="min-w-0">
          <GlassCard class="p-8 lg:p-12 prose prose-invert prose-indigo max-w-none">
            {currentDoc() ? (
              <>
                <h1 class="text-4xl font-black mb-8 gradient-text inline-block">
                  {currentDoc()?.metadata.title}
                </h1>
                <div class="markdown-content">
                  <SolidMarkdown children={currentDoc()?.content} />
                </div>
              </>
            ) : (
              <div class="text-center py-20">
                <h1 class="text-2xl font-bold text-white mb-4">Document Not Found</h1>
                <p class="text-slate-400 mb-8">The requested documentation page could not be found.</p>
                <A href="/docs" class="btn-primary">Back to Docs</A>
              </div>
            )}
          </GlassCard>
        </div>
      </main>
    </div>
  );
};

export default DocsPage;
