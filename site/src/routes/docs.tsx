import { lazy, Suspense } from "solid-js";
import { Title, Meta, Link } from "@solidjs/meta";

const DocsPage = lazy(() => import("../pages/DocsPage"));

function PageLoader() {
  return (
    <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
    </div>
  );
}

export default function Docs() {
  return (
    <>
      <Title>Documentation - OMG Package Manager</Title>
      <Meta
        name="description"
        content="Complete documentation for OMG - the fastest unified package manager for Linux. Learn installation, configuration, CLI commands, and runtime management."
      />
      <Link rel="canonical" href="https://pyro1121.com/docs" />

      <Meta property="og:title" content="OMG Documentation" />
      <Meta
        property="og:description"
        content="Learn how to use OMG - the fastest unified package manager for Linux."
      />
      <Meta property="og:url" content="https://pyro1121.com/docs" />

      <Suspense fallback={<PageLoader />}>
        <DocsPage />
      </Suspense>
    </>
  );
}
