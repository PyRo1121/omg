import { lazy, Suspense, createMemo } from "solid-js";
import { useParams } from "@solidjs/router";
import { Title, Meta, Link } from "@solidjs/meta";

const DocsPage = lazy(() => import("../../pages/DocsPage"));

function PageLoader() {
  return (
    <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
    </div>
  );
}

export default function DocsSlug() {
  const params = useParams<{ slug: string }>();

  const pageTitle = createMemo(() => {
    const slug = params.slug || "index";
    const formatted = slug
      .split("-")
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
    return `${formatted} - OMG Documentation`;
  });

  return (
    <>
      <Title>{pageTitle()}</Title>
      <Meta name="description" content={`Documentation for ${params.slug || "OMG Package Manager"}`} />
      <Link rel="canonical" href={`https://pyro1121.com/docs/${params.slug || ""}`} />

      <Suspense fallback={<PageLoader />}>
        <DocsPage />
      </Suspense>
    </>
  );
}
