import { lazy, Suspense, Show } from "solid-js";
import { Title, Meta } from "@solidjs/meta";
import { createAsync, redirect } from "@solidjs/router";
import { getRequestEvent } from "solid-js/web";
import { createAuth, CloudflareEnv } from "~/lib/auth";

const DashboardPage = lazy(() => import("../pages/DashboardPage"));

async function requireAuth() {
  "use server";
  
  const event = getRequestEvent();
  const cf = (event?.nativeEvent as any)?.context?.cloudflare?.env;
  
  if (!cf?.DB || !cf?.BETTER_AUTH_KV) {
    console.error("Auth bindings not available in dashboard route");
    throw redirect("/login");
  }
  
  const env: CloudflareEnv = {
    DB: cf.DB,
    BETTER_AUTH_KV: cf.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: cf.BETTER_AUTH_SECRET || "dev-secret-change-me",
    BETTER_AUTH_URL: cf.BETTER_AUTH_URL || "http://localhost:3000",
    GITHUB_CLIENT_ID: cf.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: cf.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: cf.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: cf.GOOGLE_CLIENT_SECRET,
  };
  
  const auth = createAuth(env);
  const session = await auth.api.getSession({
    headers: event!.request.headers,
  });
  
  if (!session?.user) {
    throw redirect("/login");
  }
  
  return session;
}

function PageLoader() {
  return (
    <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
    </div>
  );
}

export default function Dashboard() {
  const session = createAsync(() => requireAuth(), { deferStream: true });
  
  return (
    <>
      <Title>Dashboard - OMG Package Manager</Title>
      <Meta name="description" content="OMG Package Manager admin dashboard - manage licenses, analytics, and team settings." />
      <Meta name="robots" content="noindex, nofollow" />
      
      <Show when={session()} fallback={<PageLoader />}>
        <Suspense fallback={<PageLoader />}>
          <DashboardPage session={session()!} />
        </Suspense>
      </Show>
    </>
  );
}
