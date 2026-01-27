# OMG Design System: Mission Control

> A world-class design system for the OMG Admin Dashboard & CRM

## Aesthetic Direction

### Core Philosophy: "Mission Control"

The OMG dashboard is designed as a premium **command center** that conveys the precision, speed, and technical sophistication of the OMG unified package manager. The aesthetic combines:

- **Deep Space Darkness**: Rich void blacks with subtle blue undertones create depth
- **Bioluminescent Data**: Colors that "glow" against darkness, making data immediately scannable
- **Technical Precision**: Clean geometry, tabular figures, and information density
- **Living System**: Pulse animations, real-time streams, and presence indicators

### Visual Differentiation

Unlike generic admin templates, OMG Mission Control features:

1. **Distinctive Typography**: Space Grotesk for display (geometric, technical) + Plus Jakarta Sans for body (modern, readable)
2. **Semantic Color Coding**: Every color has meaning tied to health scores, lifecycle stages, risk levels
3. **Depth Through Glow**: Colored glows and shadows create visual hierarchy without traditional elevation
4. **Real-time DNA**: Built-in patterns for live data streams, presence indicators, and animated transitions

---

## Typography

### Font Stack

```css
--font-display: 'Space Grotesk', system-ui, sans-serif;  /* Headers, metrics */
--font-body: 'Plus Jakarta Sans', system-ui, sans-serif; /* Body, UI text */
--font-mono: 'JetBrains Mono', 'Fira Code', monospace;   /* Code, data */
```

### Google Fonts Import

```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600;700&family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=Space+Grotesk:wght@400;500;600;700&display=swap" rel="stylesheet">
```

### Type Scale

| Token | Size | Use Case |
|-------|------|----------|
| `text-2xs` | 10px | Micro labels, timestamps |
| `text-xs` | 12px | Badges, captions |
| `text-sm` | 14px | Body text, table cells |
| `text-base` | 16px | Default body |
| `text-lg` | 18px | Emphasized body |
| `text-xl` | 20px | Card headers |
| `text-2xl` | 24px | Section titles |
| `text-3xl` | 30px | Page subtitles |
| `text-4xl` | 36px | Page titles |
| `text-5xl` | 48px | Hero metrics |

### Typography Patterns

```jsx
// Page title
<h1 class="text-4xl font-black tracking-tight text-white font-display">
  System Command
</h1>

// Section header
<h3 class="text-xl font-black tracking-tight text-white">
  Customer CRM
</h3>

// Metric display
<span class="font-display text-5xl font-black tabular-nums text-white">
  12,847
</span>

// Label
<span class="text-xs font-bold uppercase tracking-widest text-nebula-500">
  Total Users
</span>

// Monospace data
<code class="font-mono text-sm text-nebula-300 tabular-nums">
  ses_abc123
</code>
```

---

## Color System

### Palette Philosophy

| Palette | Purpose | Primary Use |
|---------|---------|-------------|
| **Void** | Backgrounds | Surface hierarchy, depth |
| **Nebula** | Text | Content hierarchy, disabled states |
| **Indigo** | Primary | Actions, links, focus states |
| **Electric** | Accent | Energy, activated states |
| **Photon** | Secondary | Creative, secondary actions |
| **Aurora** | Success | Healthy, positive, growth |
| **Solar** | Warning | Attention, caution, review |
| **Flare** | Error | Critical, danger, urgent |
| **Plasma** | Info | Informational, new, neutral |

### Health Score Colors

```
0-20:  Critical (Flare-500)    #ef4444
21-40: Poor (Flare-400)        #f87171
41-60: Fair (Solar-500)        #f59e0b
61-80: Good (Electric-500)     #22d3d3
81-100: Excellent (Aurora-500) #10b981
```

### Lifecycle Stage Colors

| Stage | Color | Icon |
|-------|-------|------|
| New | Plasma-400 | Sparkles |
| Onboarding | Photon-400 | Rocket |
| Activated | Electric-500 | Zap |
| Engaged | Indigo-400 | Activity |
| Power User | Solar-400 | Crown |
| At Risk | Flare-400 | AlertTriangle |
| Churning | Flare-600 | TrendingDown |
| Churned | Nebula-600 | XCircle |
| Reactivated | Aurora-400 | RefreshCw |

### Tier Colors

| Tier | Color | Background |
|------|-------|------------|
| Free | Nebula-500 | Solid |
| Pro | Indigo-400 | Gradient to Purple |
| Team | Electric-400 | Gradient to Plasma |
| Enterprise | Solar-400 | Gradient to Orange |

---

## Spacing & Layout

### Spacing Scale (4px base)

```
space-1:  4px   | Tight gaps, inline spacing
space-2:  8px   | Component internal padding
space-3:  12px  | Small component gaps
space-4:  16px  | Default padding
space-6:  24px  | Card padding
space-8:  32px  | Section padding
space-10: 40px  | Large gaps
space-12: 48px  | Section margins
space-16: 64px  | Page sections
```

### Dashboard Grid

```jsx
// 4-column metric grid
<DashboardGrid columns={4} gap="lg">
  <DataCard ... />
  <DataCard ... />
  <DataCard ... />
  <DataCard ... />
</DashboardGrid>

// Mixed layout
<DashboardGrid columns={3} gap="lg">
  <GridItem span={2}>  {/* Spans 2 columns */}
    <CommandStream />
  </GridItem>
  <GridItem span={1}>
    <GlobalPresence />
  </GridItem>
</DashboardGrid>
```

### Breakpoints

```
xs:  475px   | Large phones
sm:  640px   | Small tablets
md:  768px   | Tablets
lg:  1024px  | Small desktops
xl:  1280px  | Desktops
2xl: 1536px  | Large desktops
3xl: 1920px  | Ultra-wide
```

---

## Component Library

### Health Score Visualizations

```jsx
// Ring gauge (default)
<HealthScore score={85} size="lg" showLabel animated />

// Progress bar
<HealthScore score={72} variant="bar" showLabel showSegments />

// Compact badge
<HealthScore score={45} variant="badge" showTrend trend={-5} />

// Half-circle gauge
<HealthScore score={90} variant="gauge" size="xl" />
```

### Lifecycle Badges

```jsx
// Standard badge
<LifecycleBadge stage="power_user" showIcon />

// Progress indicator
<LifecycleProgress currentStage="engaged" showLabels />

// Timeline view
<LifecycleTimeline 
  currentStage="activated"
  stageHistory={[
    { stage: 'new', date: 'Jan 15' },
    { stage: 'onboarding', date: 'Jan 18' },
    { stage: 'activated', date: 'Jan 25' }
  ]}
/>
```

### Real-time Indicators

```jsx
// Live pulse
<LiveIndicator label="Live" variant="pulse" color="success" />

// Stream counter
<StreamCounter count={1247} label="Events" rate={12} />

// Presence dot
<PresenceIndicator status="online" label="John Doe" />

// Data stream
<DataStream 
  items={[
    { id: '1', content: 'omg search firefox', type: 'success' },
    { id: '2', content: 'omg install vim', type: 'info' }
  ]}
/>
```

### Data Cards

```jsx
// Basic stat card
<DataCard
  title="Total Users"
  value="12,847"
  icon={<Users />}
  trend={{ value: 8.2, direction: 'up', period: 'vs last month' }}
  accent="indigo"
/>

// With sparkline
<SparklineCard
  title="Revenue"
  value="$45,230"
  data={[10, 15, 12, 25, 30, 28, 35]}
  sparklineColor="#10b981"
/>
```

### Tier Badges

```jsx
// Badge
<TierBadge tier="enterprise" size="md" glowing />

// Card format
<TierBadge tier="team" variant="card" />

// Comparison
<TierComparison currentTier="pro" recommendedTier="team" />
```

### Risk Indicators

```jsx
// Badge
<RiskIndicator level="high" probability={75} />

// Progress bar
<RiskIndicator level="critical" variant="bar" probability={92} showLabel />

// Segments list
<RiskSegments 
  segments={[
    { level: 'critical', count: 5, tier: 'enterprise', avgCommands: 12 },
    { level: 'high', count: 23, tier: 'pro', avgCommands: 45 }
  ]}
  onSegmentClick={(level) => console.log(level)}
/>
```

### CRM Components

```jsx
// Notes list
<NotesList
  notes={notes}
  onAddNote={(content, type) => createNote(content, type)}
  onDeleteNote={(id) => deleteNote(id)}
/>

// Tags manager
<TagsManager
  assignedTags={customerTags}
  availableTags={allTags}
  onAssign={(id) => assignTag(id)}
  onRemove={(id) => removeTag(id)}
  onCreate={(name, color) => createTag(name, color)}
/>

// Task card
<TaskCard
  task={{
    id: '1',
    type: 'renewal',
    title: 'Schedule renewal call',
    dueDate: 'Tomorrow',
    completed: false,
    priority: 'high'
  }}
  onToggle={(id) => toggleTask(id)}
/>

// Communication timeline
<CommunicationTimeline communications={history} />
```

### Layout Components

```jsx
// Page header
<PageHeader
  title="System Command"
  subtitle="Global infrastructure telemetry"
  breadcrumbs={[
    { label: 'Dashboard', href: '/' },
    { label: 'Admin' }
  ]}
  actions={<Button>Export</Button>}
/>

// Tab navigation
<TabNavigation
  tabs={[
    { id: 'overview', label: 'Overview', icon: <Activity /> },
    { id: 'crm', label: 'CRM', icon: <Users />, badge: 23 }
  ]}
  activeTab={activeTab}
  onChange={setActiveTab}
  variant="pills"
/>

// Section
<Section
  title="Customer Health"
  subtitle="Real-time engagement metrics"
  action={<RefreshButton />}
  variant="card"
  collapsible
>
  {/* Content */}
</Section>

// Drawer
<Drawer
  open={drawerOpen}
  onClose={() => setDrawerOpen(false)}
  title="Customer Detail"
  subtitle="360° view"
  width="2xl"
>
  {/* Drawer content */}
</Drawer>
```

---

## Animation System

### Timing

```css
--duration-fast:    100ms  /* Hover states */
--duration-normal:  200ms  /* Standard transitions */
--duration-slow:    300ms  /* Complex animations */
--duration-slower:  500ms  /* Page transitions */
```

### Easing

```css
--ease-smooth: cubic-bezier(0.23, 1, 0.32, 1)   /* Most interactions */
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1) /* Bouncy feedback */
--ease-swift:  cubic-bezier(0.16, 1, 0.3, 1)    /* Quick, snappy */
```

### Keyframe Animations

| Animation | Use Case |
|-----------|----------|
| `animate-pulse-glow` | Active/selected states with glow |
| `animate-pulse-slow` | Subtle breathing effect |
| `animate-float` | Floating elements |
| `animate-shimmer` | Loading/skeleton states |
| `animate-gauge-fill` | Health score ring fill |
| `animate-stream-in` | List item entrance |
| `animate-slide-in-right` | Drawer entrance |
| `animate-fade-up` | Content entrance |

---

## Interaction Patterns

### Health Score Changes

```jsx
// Score change triggers:
// 1. Ring fills with easing animation
// 2. Color transitions smoothly
// 3. Glow pulses briefly
// 4. Optional trend indicator appears

<HealthScore 
  score={newScore} 
  animated 
  showTrend 
  trend={delta}
/>
```

### Real-time Updates

```jsx
// New stream items:
// 1. Slide in from left with scale
// 2. Previous items shift down
// 3. Old items fade out at bottom
// 4. Counter increments with lerp

<DataStream items={liveItems} maxVisible={5} />
<StreamCounter count={total} rate={perSecond} />
```

### Drawer/Modal Patterns

```jsx
// Open:
// 1. Backdrop fades in with blur
// 2. Panel slides from right
// 3. Content fades in staggered

// Close:
// 1. Content fades out
// 2. Panel slides out
// 3. Backdrop fades
```

### Filter/Search

```jsx
// 1. Results filter immediately (optimistic)
// 2. Loading skeleton for async
// 3. Stagger animate results in
// 4. Empty state if no results
```

---

## Accessibility

### Color Contrast

All text meets WCAG AA standards:
- Regular text: 4.5:1 contrast ratio
- Large text: 3:1 contrast ratio
- Interactive elements: Visible focus states

### Focus States

```css
/* Keyboard focus */
focus:outline-none focus:ring-2 focus:ring-indigo-500/50 focus:ring-offset-2 focus:ring-offset-void-950
```

### Motion

```css
/* Respect reduced motion */
@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## Implementation Checklist

### Required Google Fonts

```html
<link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600;700&family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=Space+Grotesk:wght@400;500;600;700&display=swap" rel="stylesheet">
```

### CSS Import

```css
@import './design-system/tokens.css';
```

### Tailwind Config

```ts
import { omgPreset } from './design-system';

export default {
  presets: [omgPreset],
  // ... rest of config
};
```

### Component Import

```tsx
import { 
  HealthScore,
  LifecycleBadge,
  LiveIndicator,
  DataCard,
  TierBadge,
  RiskIndicator,
  DashboardGrid,
  Section
} from './design-system';
```
