import { lazy, Suspense } from "solid-js";
import { Title, Meta } from "@solidjs/meta";

const DashboardPage = lazy(() => import("../pages/DashboardPage"));

function PageLoader() {
  return (
    <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
    </div>
  );
}

export default function Dashboard() {
  return (
    <>
      <Title>Dashboard - OMG Package Manager</Title>
      <Meta name="description" content="OMG Package Manager admin dashboard - manage licenses, analytics, and team settings." />
      <Meta name="robots" content="noindex, nofollow" />
      
      <Suspense fallback={<PageLoader />}>
        <DashboardPage />
      </Suspense>
    </>
  );
}
