# Dashboard Improvements Implementation Plan

**Created:** 2026-01-26
**Based On:** Design Review `dashboard-admin-20260126`
**Status:** Ready for Implementation

---

## Executive Summary

This plan outlines the implementation of missing features identified in the design review. The backend already provides all necessary endpoints - our work is purely frontend integration to surface existing data.

**Key Insight:** Backend engineering is complete. We're adding UI components to visualize data the API already provides.

---

## Architecture Overview

### Current State
```typescript
// AdminDashboard.tsx uses only 3 hooks:
useAdminDashboard()    // Basic overview stats
useAdminFirehose()     // Event stream
useAdminCRMUsers()     // User list with pagination
```

### Target State
```typescript
// Complete data utilization:
useAdminDashboard()           // Overview stats
useAdminFirehose()            // Event stream
useAdminCRMUsers()            // User list
useAdminAdvancedMetrics()     // NEW: Engagement, churn, expansion
useAdminCohorts()             // NEW: Retention cohorts
useAdminCustomerHealth()      // NEW: Per-customer health scores
useAdminCustomerNotes()       // NEW: CRM notes
useAdminCustomerTags()        // NEW: CRM tags
```

---

## Phase 5A: Quick Wins (1-2 days)

### Task 1: Export Menu Enhancement
**Effort:** 2-3 hours
**Files:** `src/components/dashboard/AdminDashboard.tsx`

**Current:**
```typescript
// Line 75-78
<button class="...">
  <Download size={16} />
  Export Data
</button>
```

**Implementation:**
1. Replace single button with dropdown menu
2. Add handlers for each export type
3. Use existing API functions

**Code:**
```typescript
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/solidjs';

// Replace export button with:
<Menu>
  <MenuButton class="flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08]">
    <Download size={16} />
    Export Data
  </MenuButton>
  <MenuItems class="absolute right-0 mt-2 w-56 origin-top-right rounded-xl border border-white/10 bg-[#0d0d0e] p-1 shadow-xl">
    <MenuItem>
      {({ active }) => (
        <button
          onClick={() => exportUsers()}
          class={`${active ? 'bg-white/5' : ''} group flex w-full items-center rounded-lg px-3 py-2 text-sm text-white`}
        >
          Users (CSV)
        </button>
      )}
    </MenuItem>
    <MenuItem>
      {({ active }) => (
        <button
          onClick={() => exportUsage()}
          class={`${active ? 'bg-white/5' : ''} group flex w-full items-center rounded-lg px-3 py-2 text-sm text-white`}
        >
          Usage (CSV)
        </button>
      )}
    </MenuItem>
    <MenuItem>
      {({ active }) => (
        <button
          onClick={() => exportAudit()}
          class={`${active ? 'bg-white/5' : ''} group flex w-full items-center rounded-lg px-3 py-2 text-sm text-white`}
        >
          Audit Log (CSV)
        </button>
      )}
    </MenuItem>
  </MenuItems>
</Menu>

// Add handlers:
const exportUsers = async () => {
  const data = await api.exportUsers();
  downloadCSV(data, 'users.csv');
};

const exportUsage = async () => {
  const data = await api.exportUsage();
  downloadCSV(data, 'usage.csv');
};

const exportAudit = async () => {
  const data = await api.exportAudit();
  downloadCSV(data, 'audit.csv');
};

const downloadCSV = (data: string, filename: string) => {
  const blob = new Blob([data], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
};
```

**API Integration:**
```typescript
// src/lib/api.ts - Add exports
export async function exportUsers(): Promise<string> {
  return await authenticatedRequest('/api/admin/export-users');
}

export async function exportUsage(): Promise<string> {
  return await authenticatedRequest('/api/admin/export-usage');
}

export async function exportAudit(): Promise<string> {
  return await authenticatedRequest('/api/admin/export-audit');
}
```

### Task 2: Health Score in CRM Table
**Effort:** 1-2 hours
**Files:** `src/components/dashboard/AdminDashboard.tsx`

**Current CRM columns (line 248-257):**
- User
- Tier
- Status
- Machines
- Commands
- Joined

**Add Health column after Status:**
```typescript
// Line 252 - Add new column header
<th class="px-6 py-4">Health</th>

// Line 288 - Add new column data (after Status cell)
<td class="px-6 py-4">
  <HealthBadge
    score={user.engagement_score}
    stage={user.lifecycle_stage}
  />
</td>
```

**Create HealthBadge component:**
```typescript
// src/components/dashboard/admin/HealthBadge.tsx
import { Component } from 'solid-js';

interface HealthBadgeProps {
  score: number;
  stage: string;
}

export const HealthBadge: Component<HealthBadgeProps> = (props) => {
  const getHealthColor = (score: number) => {
    if (score >= 80) return 'text-emerald-400 bg-emerald-500/10';
    if (score >= 60) return 'text-cyan-400 bg-cyan-500/10';
    if (score >= 40) return 'text-amber-400 bg-amber-500/10';
    return 'text-rose-400 bg-rose-500/10';
  };

  const getStageIcon = (stage: string) => {
    switch (stage) {
      case 'power_user': return '⚡';
      case 'active': return '✓';
      case 'at_risk': return '⚠️';
      case 'churned': return '×';
      case 'new': return '✨';
      default: return '·';
    }
  };

  return (
    <div class="flex items-center gap-2">
      <div class={`rounded-full px-2 py-0.5 text-xs font-bold ${getHealthColor(props.score)}`}>
        {props.score}
      </div>
      <span class="text-xs" title={props.stage}>
        {getStageIcon(props.stage)}
      </span>
    </div>
  );
};
```

**Note:** The `engagement_score` and `lifecycle_stage` fields are already returned by the `useAdminCRMUsers` query (see admin.ts:264-271), so no API changes needed.

---

## Phase 5B: Medium Impact (3-5 days)

### Task 3: Customer Notes & Tags in Detail Drawer
**Effort:** 1 day
**Files:**
- `src/components/dashboard/admin/CustomerDetailDrawer.tsx` (enhance)
- `src/components/dashboard/admin/NotesSection.tsx` (new)
- `src/components/dashboard/admin/TagsSection.tsx` (new)
- `src/lib/api-hooks.ts` (add hooks)
- `src/lib/api.ts` (add endpoints)

**API Hooks:**
```typescript
// src/lib/api-hooks.ts

export function useAdminCustomerNotes(customerId: Accessor<string | null>) {
  return createQuery(() => ({
    queryKey: ['admin', 'customer-notes', customerId()],
    queryFn: () => api.getAdminCustomerNotes(customerId()!),
    enabled: !!customerId(),
  }));
}

export function useAdminCustomerTags(customerId: Accessor<string | null>) {
  return createQuery(() => ({
    queryKey: ['admin', 'customer-tags', customerId()],
    queryFn: () => api.getAdminCustomerTags(customerId()!),
    enabled: !!customerId(),
  }));
}

export function useCreateCustomerNote() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: ({ customerId, note }: { customerId: string; note: string }) =>
      api.createAdminCustomerNote(customerId, note),
    onSuccess: (_, { customerId }) => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'customer-notes', customerId] });
    },
  }));
}

export function useDeleteCustomerNote() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: ({ customerId, noteId }: { customerId: string; noteId: string }) =>
      api.deleteAdminCustomerNote(customerId, noteId),
    onSuccess: (_, { customerId }) => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'customer-notes', customerId] });
    },
  }));
}
```

**NotesSection Component:**
```typescript
// src/components/dashboard/admin/NotesSection.tsx
import { Component, createSignal, For, Show } from 'solid-js';
import { MessageSquare, Pin, Trash2 } from 'lucide-solid';
import * as api from '../../../lib/api';

interface Note {
  id: string;
  note: string;
  pinned: boolean;
  created_at: string;
  created_by: string;
}

interface NotesSectionProps {
  customerId: string;
  notes: Note[];
  onAddNote: (note: string) => void;
  onDeleteNote: (noteId: string) => void;
}

export const NotesSection: Component<NotesSectionProps> = (props) => {
  const [newNote, setNewNote] = createSignal('');
  const [isAdding, setIsAdding] = createSignal(false);

  const handleAdd = () => {
    if (newNote().trim()) {
      props.onAddNote(newNote());
      setNewNote('');
      setIsAdding(false);
    }
  };

  const sortedNotes = () => [...props.notes].sort((a, b) => {
    if (a.pinned && !b.pinned) return -1;
    if (!a.pinned && b.pinned) return 1;
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  return (
    <div class="rounded-xl border border-white/10 bg-white/5 p-6">
      <div class="mb-4 flex items-center justify-between">
        <h4 class="flex items-center gap-2 text-lg font-bold text-white">
          <MessageSquare size={18} />
          Customer Notes
        </h4>
        <button
          onClick={() => setIsAdding(!isAdding())}
          class="rounded-lg bg-indigo-500/20 px-3 py-1.5 text-sm font-bold text-indigo-400 transition-colors hover:bg-indigo-500/30"
        >
          {isAdding() ? 'Cancel' : '+ Add Note'}
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="mb-4">
          <textarea
            value={newNote()}
            onInput={(e) => setNewNote(e.currentTarget.value)}
            placeholder="Add a note about this customer..."
            class="w-full rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/50"
            rows={3}
          />
          <div class="mt-2 flex justify-end gap-2">
            <button
              onClick={handleAdd}
              class="rounded-lg bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-indigo-600"
            >
              Save Note
            </button>
          </div>
        </div>
      </Show>

      <div class="space-y-3">
        <Show when={props.notes.length === 0}>
          <p class="py-8 text-center text-sm text-slate-500">No notes yet</p>
        </Show>

        <For each={sortedNotes()}>
          {(note) => (
            <div class="group rounded-lg border border-white/5 bg-white/5 p-4">
              <div class="mb-2 flex items-start justify-between">
                <div class="flex items-center gap-2">
                  <Show when={note.pinned}>
                    <Pin size={14} class="text-amber-400" />
                  </Show>
                  <span class="text-xs text-slate-400">
                    {api.formatRelativeTime(note.created_at)} by {note.created_by}
                  </span>
                </div>
                <button
                  onClick={() => props.onDeleteNote(note.id)}
                  class="opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <Trash2 size={14} class="text-rose-400 hover:text-rose-300" />
                </button>
              </div>
              <p class="text-sm text-white">{note.note}</p>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};
```

**TagsSection Component:**
```typescript
// src/components/dashboard/admin/TagsSection.tsx
import { Component, createSignal, For, Show } from 'solid-js';
import { Tag, X } from 'lucide-solid';

interface CustomerTag {
  id: string;
  tag_name: string;
  tag_color: string;
}

interface TagsSectionProps {
  customerId: string;
  tags: CustomerTag[];
  onAddTag: (tagName: string) => void;
  onRemoveTag: (tagId: string) => void;
}

export const TagsSection: Component<TagsSectionProps> = (props) => {
  const [isAdding, setIsAdding] = createSignal(false);
  const [newTag, setNewTag] = createSignal('');

  const handleAdd = () => {
    if (newTag().trim()) {
      props.onAddTag(newTag());
      setNewTag('');
      setIsAdding(false);
    }
  };

  return (
    <div class="rounded-xl border border-white/10 bg-white/5 p-6">
      <div class="mb-4 flex items-center justify-between">
        <h4 class="flex items-center gap-2 text-lg font-bold text-white">
          <Tag size={18} />
          Tags
        </h4>
        <button
          onClick={() => setIsAdding(!isAdding())}
          class="rounded-lg bg-purple-500/20 px-3 py-1.5 text-sm font-bold text-purple-400 transition-colors hover:bg-purple-500/30"
        >
          {isAdding() ? 'Cancel' : '+ Add Tag'}
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="mb-4">
          <input
            type="text"
            value={newTag()}
            onInput={(e) => setNewTag(e.currentTarget.value)}
            onKeyPress={(e) => e.key === 'Enter' && handleAdd()}
            placeholder="Enter tag name..."
            class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-purple-500/50"
          />
        </div>
      </Show>

      <div class="flex flex-wrap gap-2">
        <Show when={props.tags.length === 0}>
          <p class="w-full py-4 text-center text-sm text-slate-500">No tags yet</p>
        </Show>

        <For each={props.tags}>
          {(tag) => (
            <div
              class="group flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium"
              style={{ background: `${tag.tag_color}20`, color: tag.tag_color }}
            >
              {tag.tag_name}
              <button
                onClick={() => props.onRemoveTag(tag.id)}
                class="opacity-0 transition-opacity group-hover:opacity-100"
              >
                <X size={14} />
              </button>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};
```

**Enhanced CustomerDetailDrawer:**
```typescript
// src/components/dashboard/admin/CustomerDetailDrawer.tsx
// Add imports and integrate sections

import { NotesSection } from './NotesSection';
import { TagsSection } from './TagsSection';
import { useAdminCustomerNotes, useAdminCustomerTags, useCreateCustomerNote, useDeleteCustomerNote } from '../../../lib/api-hooks';

// In component:
const notesQuery = useAdminCustomerNotes(() => props.userId);
const tagsQuery = useAdminCustomerTags(() => props.userId);
const createNoteMutation = useCreateCustomerNote();
const deleteNoteMutation = useDeleteCustomerNote();

// Add sections in drawer content (before machines/usage):
<NotesSection
  customerId={props.userId!}
  notes={notesQuery.data || []}
  onAddNote={(note) => createNoteMutation.mutate({ customerId: props.userId!, note })}
  onDeleteNote={(noteId) => deleteNoteMutation.mutate({ customerId: props.userId!, noteId })}
/>

<TagsSection
  customerId={props.userId!}
  tags={tagsQuery.data || []}
  onAddTag={(tagName) => assignTagMutation.mutate({ customerId: props.userId!, tagName })}
  onRemoveTag={(tagId) => removeTagMutation.mutate({ customerId: props.userId!, tagId })}
/>
```

### Task 4: Cohort Analysis Visualization
**Effort:** 1 day
**Files:**
- `src/components/dashboard/admin/CohortAnalysis.tsx` (new)
- `src/lib/api-hooks.ts` (add hook)

**API Hook:**
```typescript
export function useAdminCohorts() {
  return createQuery(() => ({
    queryKey: ['admin', 'cohorts'],
    queryFn: () => api.getAdminCohorts(),
  }));
}
```

**Component:**
```typescript
// src/components/dashboard/admin/CohortAnalysis.tsx
import { Component, For, Show } from 'solid-js';
import { useAdminCohorts } from '../../../lib/api-hooks';
import { TrendingUp } from 'lucide-solid';

interface CohortData {
  cohort_month: string;
  month_index: number;
  active_users: number;
  retention_rate: number;
}

export const CohortAnalysis: Component = () => {
  const cohortsQuery = useAdminCohorts();

  const cohortMap = () => {
    const data = cohortsQuery.data || [];
    const map = new Map<string, CohortData[]>();

    data.forEach(item => {
      if (!map.has(item.cohort_month)) {
        map.set(item.cohort_month, []);
      }
      map.get(item.cohort_month)!.push(item);
    });

    return Array.from(map.entries()).sort((a, b) => b[0].localeCompare(a[0])).slice(0, 12);
  };

  const getRetentionColor = (rate: number) => {
    if (rate >= 80) return 'bg-emerald-500';
    if (rate >= 60) return 'bg-cyan-500';
    if (rate >= 40) return 'bg-amber-500';
    if (rate >= 20) return 'bg-orange-500';
    return 'bg-rose-500';
  };

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6 flex items-center gap-3">
        <TrendingUp size={24} class="text-purple-400" />
        <div>
          <h3 class="text-2xl font-black tracking-tight text-white">Cohort Retention Analysis</h3>
          <p class="text-sm text-slate-500">User retention by signup month</p>
        </div>
      </div>

      <Show when={cohortsQuery.isLoading}>
        <div class="py-12 text-center text-slate-400">Loading cohort data...</div>
      </Show>

      <Show when={cohortsQuery.isSuccess}>
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-white/10">
                <th class="px-4 py-3 text-left text-xs font-bold text-slate-400 uppercase">Cohort</th>
                <For each={Array.from({ length: 12 }, (_, i) => i)}>
                  {(month) => (
                    <th class="px-2 py-3 text-center text-xs font-bold text-slate-400">M{month}</th>
                  )}
                </For>
              </tr>
            </thead>
            <tbody>
              <For each={cohortMap()}>
                {([cohortMonth, data]) => (
                  <tr class="border-b border-white/5 hover:bg-white/5">
                    <td class="px-4 py-3 font-mono text-xs text-white">{cohortMonth}</td>
                    <For each={Array.from({ length: 12 }, (_, i) => i)}>
                      {(monthIndex) => {
                        const cohortData = data.find(d => d.month_index === monthIndex);
                        return (
                          <td class="px-2 py-3 text-center">
                            <Show when={cohortData} fallback={<span class="text-slate-700">-</span>}>
                              <div
                                class={`inline-block rounded px-2 py-1 text-xs font-bold text-white ${getRetentionColor(cohortData!.retention_rate)}`}
                                title={`${cohortData!.active_users} users (${cohortData!.retention_rate}%)`}
                              >
                                {cohortData!.retention_rate}%
                              </div>
                            </Show>
                          </td>
                        );
                      }}
                    </For>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
};
```

**Integration:** Add to Analytics tab in AdminDashboard.tsx

### Task 5: Enhanced Audit Log Filtering
**Effort:** 0.5 days
**Files:** `src/components/dashboard/admin/AuditLogTab.tsx`

Add comprehensive filtering UI to existing AuditLogTab component with action type, user, date range, and IP address filters.

---

## Phase 5C: High Impact (1-2 weeks)

### Task 6: Complete Insights Dashboard
**Effort:** 1 week
**Files:**
- `src/components/dashboard/admin/insights/InsightsTab.tsx` (main)
- `src/components/dashboard/admin/insights/EngagementMetrics.tsx`
- `src/components/dashboard/admin/insights/ChurnRiskSegments.tsx`
- `src/components/dashboard/admin/insights/ExpansionOpportunities.tsx`
- `src/components/dashboard/admin/insights/TimeToValueMetrics.tsx`
- `src/components/dashboard/admin/insights/FeatureAdoptionChart.tsx`
- `src/components/dashboard/admin/insights/CommandHeatmap.tsx`
- `src/components/dashboard/admin/insights/RuntimeAdoptionChart.tsx`
- `src/lib/api-hooks.ts` (add useAdminAdvancedMetrics)
- `src/components/dashboard/AdminDashboard.tsx` (add insights tab)

**This is the HIGHEST ROI task** - surfaces 10+ critical business intelligence metrics.

**API Hook:**
```typescript
export function useAdminAdvancedMetrics() {
  return createQuery(() => ({
    queryKey: ['admin', 'advanced-metrics'],
    queryFn: () => api.getAdminAdvancedMetrics(),
    staleTime: 5 * 60 * 1000, // 5 minutes - this is expensive data
  }));
}
```

**Main Tab:**
```typescript
// src/components/dashboard/admin/insights/InsightsTab.tsx
import { Component, Show } from 'solid-js';
import { useAdminAdvancedMetrics } from '../../../../lib/api-hooks';
import { EngagementMetrics } from './EngagementMetrics';
import { ChurnRiskSegments } from './ChurnRiskSegments';
import { ExpansionOpportunities } from './ExpansionOpportunities';
import { TimeToValueMetrics } from './TimeToValueMetrics';
import { FeatureAdoptionChart } from './FeatureAdoptionChart';
import { CommandHeatmap } from './CommandHeatmap';
import { RuntimeAdoptionChart } from './RuntimeAdoptionChart';
import { CardSkeleton } from '../../../ui/Skeleton';

export const InsightsTab: Component = () => {
  const metricsQuery = useAdminAdvancedMetrics();

  return (
    <div class="animate-in fade-in slide-in-from-bottom-4 space-y-8 duration-500">
      <div>
        <h2 class="text-3xl font-black tracking-tight text-white">Business Intelligence</h2>
        <p class="mt-2 text-sm text-slate-400">
          Advanced analytics, customer health, and growth opportunities
        </p>
      </div>

      <Show when={metricsQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={metricsQuery.isSuccess && metricsQuery.data}>
        <div class="space-y-8">
          {/* Engagement Overview */}
          <EngagementMetrics data={metricsQuery.data!.engagement} />

          {/* Critical Business Metrics - Two Column */}
          <div class="grid gap-6 lg:grid-cols-2">
            <ChurnRiskSegments data={metricsQuery.data!.churn_risk_segments} />
            <ExpansionOpportunities data={metricsQuery.data!.expansion_opportunities} />
          </div>

          {/* Product Insights */}
          <TimeToValueMetrics data={metricsQuery.data!.time_to_value} />

          {/* Feature Analytics - Two Column */}
          <div class="grid gap-6 lg:grid-cols-2">
            <FeatureAdoptionChart data={metricsQuery.data!.feature_adoption} />
            <CommandHeatmap data={metricsQuery.data!.command_heatmap} />
          </div>

          {/* Runtime Adoption */}
          <RuntimeAdoptionChart data={metricsQuery.data!.runtime_adoption} />
        </div>
      </Show>

      <Show when={metricsQuery.isError}>
        <div class="rounded-xl border border-rose-500/30 bg-rose-500/10 p-8 text-center">
          <p class="text-rose-400">Failed to load advanced metrics</p>
          <p class="mt-2 text-sm text-slate-400">{metricsQuery.error?.message}</p>
        </div>
      </Show>
    </div>
  );
};
```

**Individual components would follow similar patterns to existing analytics components (DocsAnalytics.tsx, RevenueTab.tsx) with appropriate visualizations for each metric type.**

**Add to AdminDashboard:**
```typescript
// Line 31: Update type
type AdminTab = 'overview' | 'crm' | 'analytics' | 'revenue' | 'audit' | 'insights';

// Line 88: Add tab button
<TabButton id="insights" icon={Lightbulb} label="Insights" />

// Line 371: Add match case
<Match when={activeTab() === 'insights'}>
  <InsightsTab />
</Match>
```

---

## Testing Strategy

### Unit Tests
- Test each new component in isolation
- Mock API responses
- Test loading/error states

### Integration Tests
- Test complete user flows
- Test mutations (notes, tags)
- Test export functionality

### Visual Regression
- Screenshot comparisons for new UI
- Mobile responsiveness checks

### Performance
- Verify query caching working
- Check bundle size impact
- Monitor re-render counts

---

## Rollout Strategy

### Stage 1: Internal Testing (Quick Wins)
- Deploy Phase 5A to staging
- Internal team validation
- Fix any issues

### Stage 2: Beta (Medium Impact)
- Deploy Phase 5B to staging
- Select beta customers test CRM features
- Gather feedback

### Stage 3: Full Release (High Impact)
- Deploy Phase 5C Insights dashboard
- Monitor performance
- Document new features

---

## Success Metrics

### User Adoption
- % of admins using Insights tab weekly
- % of admins adding customer notes
- % of customers tagged

### Business Impact
- Time to identify at-risk customers (reduce from N/A to < 1 day)
- Expansion conversations started (track via tags/notes)
- Churn prevention actions taken

### Technical
- No increase in error rates
- Page load time < 2s for all tabs
- API response times < 500ms

---

## Risks and Mitigations

### Risk: Performance impact from advanced metrics endpoint
**Mitigation:**
- 5-minute staleTime on query
- Consider adding Redis caching on backend
- Monitor query execution time

### Risk: User confusion with new features
**Mitigation:**
- Add tooltips explaining metrics
- Create internal documentation
- Gradual rollout with training

### Risk: Backend endpoint issues
**Mitigation:**
- All endpoints already exist and tested
- Add comprehensive error boundaries
- Graceful degradation if metrics unavailable

---

## Open Questions

1. Should health scores be visible to non-admin users for their own account?
2. What level of permissions for customer notes? (All admins or role-based?)
3. Should we add real-time updates for command stream in Insights?
4. Export format preferences? (Currently CSV, want JSON/Excel?)

---

## Maintenance Considerations

### Documentation
- Update API documentation with new hooks
- Create user guide for CRM features
- Document metric calculations

### Monitoring
- Add Sentry tracking for new components
- Monitor advanced metrics endpoint performance
- Track feature adoption in analytics

### Future Enhancements
- Real-time updates via WebSocket
- Custom dashboard builder
- Saved filters/views
- Scheduled reports

---

**Status:** Ready for implementation. All backend endpoints exist and are production-ready. Frontend work is purely additive - no breaking changes to existing functionality.
