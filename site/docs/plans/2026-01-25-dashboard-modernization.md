# OMG Dashboard Modernization Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Modernize the Admin Dashboard by refactoring the "God Component", implementing TanStack Query for data fetching, and introducing TanStack Table for data grids.

**Architecture:** Moving from a monolithic, polling-based architecture to a composable, query-driven state management system. We will use specific UI primitives (Kobalte/SolidUI) for consistency and TanStack libraries for logic.

**Tech Stack:** SolidJS, TanStack Query, TanStack Table, Solid ApexCharts, Kobalte/SolidUI (Components).

---

## Phase 1: Foundation & Dependencies

### Task 1: Install Modern Dependencies
**Goal:** Set up the new libraries required for the modernization.

**Files:**
- Modify: `site/package.json`

**Step 1: Install Packages**
```bash
cd site
npm install @tanstack/solid-query @tanstack/solid-table solid-apexcharts apexcharts @kobalte/core lucide-solid
```

**Step 2: Commit**
```bash
git add package.json package-lock.json
git commit -m "chore: install tanstack query, table, charts and kobalte"
```

---

## Phase 2: Data Layer Upgrade (TanStack Query)

### Task 2: Setup QueryClient & API Wrapper
**Goal:** Replace manual `fetch` calls with a typed API client and a global QueryClient provider.

**Files:**
- Modify: `site/src/index.tsx` (Add QueryClientProvider)
- Create: `site/src/lib/query.ts` (Client instance)
- Create: `site/src/lib/api-hooks.ts` (Reusable queries)

**Step 1: Create Query Client**
- `query.ts`: Export `queryClient = new QueryClient(...)` with default options (staleTime: 5000).

**Step 2: Add Provider**
- `index.tsx`: Wrap the app in `<QueryClientProvider client={queryClient}>`.

**Step 3: Create Hooks**
- `api-hooks.ts`: create `useTeamAnalytics`, `useFleetStatus`, `useAdminEvents`.
- Move the `fetch` logic from `TeamAnalytics.tsx` and `AdminDashboard.tsx` into these query functions.

**Step 4: Commit**
```bash
git add site/src/index.tsx site/src/lib/query.ts site/src/lib/api-hooks.ts
git commit -m "feat: setup tanstack query client and api hooks"
```

---

## Phase 3: "God Component" Decomposition

### Task 3: Extract Analytics Cards
**Goal:** Break `TeamAnalytics.tsx` logic into reusable, display-only components.

**Files:**
- Create: `site/src/components/dashboard/analytics/StatCard.tsx`
- Create: `site/src/components/dashboard/analytics/RoiChart.tsx`
- Create: `site/src/components/dashboard/analytics/SecurityScore.tsx`

**Step 1: Create StatCard**
- A generic GlassCard variant accepting `title`, `value`, `trend` (up/down), and `icon`.

**Step 2: Create RoiChart**
- Use `solid-apexcharts` to implement the Area chart currently hardcoded in `TeamAnalytics`.

**Step 3: Create SecurityScore**
- A radial bar or progress component showing the security posture score.

**Step 4: Commit**
```bash
git add site/src/components/dashboard/analytics/
git commit -m "refactor: extract reusable analytics components"
```

### Task 4: Reassemble TeamAnalytics
**Goal:** Rewrite `TeamAnalytics.tsx` to use the new hooks and components.

**Files:**
- Modify: `site/src/components/dashboard/TeamAnalytics.tsx`

**Step 1: Implement Queries**
- Replace `onMount` fetch logic with `const query = useTeamAnalytics()`.

**Step 2: Replace JSX**
- Swap hardcoded HTML with `<StatCard />`, `<RoiChart />`, etc.
- Remove the 1000+ lines of inline logic.

**Step 3: Commit**
```bash
git add site/src/components/dashboard/TeamAnalytics.tsx
git commit -m "refactor: modernize TeamAnalytics with query hooks and components"
```

---

## Phase 4: Modern Data Grid (TanStack Table)

### Task 5: Implement Fleet Table
**Goal:** Replace the `MachinesView` list with a powerful sortable table.

**Files:**
- Create: `site/src/components/dashboard/tables/FleetTable.tsx`
- Modify: `site/src/components/dashboard/MachinesView.tsx`

**Step 1: Create FleetTable**
- Use `createSolidTable()` from `@tanstack/solid-table`.
- Columns: "Hostname", "OS", "Version", "Last Seen", "Status".
- Features: Sorting, simple pagination.
- UI: Use `GlassCard` and a standard HTML table styled with Tailwind.

**Step 2: Update MachinesView**
- Use `useFleetStatus()` hook.
- Render `<FleetTable data={query.data} />`.

**Step 3: Commit**
```bash
git add site/src/components/dashboard/tables/FleetTable.tsx site/src/components/dashboard/MachinesView.tsx
git commit -m "feat: implement tanstack table for fleet view"
```

---

## Phase 5: Real-Time Admin Dashboard

### Task 6: Optimistic UI for Admin
**Goal:** Improve the `AdminDashboard` responsiveness.

**Files:**
- Modify: `site/src/components/dashboard/AdminDashboard.tsx`

**Step 1: Update Queries**
- Use `useAdminEvents` hook with `refetchInterval` (replacing manual `setInterval`).

**Step 2: UI Cleanup**
- Use the new `<StatCard>` components for the top metrics.
- Ensure the "Firehose" list uses `transition-group` (or Solid's `<Transition>`) for smooth entry of new events.

**Step 3: Commit**
```bash
git add site/src/components/dashboard/AdminDashboard.tsx
git commit -m "refactor: update admin dashboard to use query polling and new components"
```
