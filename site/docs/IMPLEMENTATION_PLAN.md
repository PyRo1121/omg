# Enterprise Telemetry Implementation Plan

> **Practical roadmap with database schemas, API endpoints, and UI components**

---

## Quick Wins (This Week)

These features provide immediate value with minimal development time:

### 1. Activity Heatmap (1-2 days)

**Value:** Visualize when teams are most active

**UI Component:**
```tsx
<ActivityHeatmap 
  data={dailyUsage} 
  weekStart="monday"
  showWeekends={false}
/>
```

**Displays:** 
- 7x24 grid (days × hours)
- Color intensity = command volume
- Hover shows exact count
- Identifies peak productivity hours

**Data Required:** Already have `usage_daily.date` + need hour breakdown

**Database Addition:**
```sql
ALTER TABLE usage_daily ADD COLUMN hour_distribution TEXT;
-- JSON: {"00": 12, "01": 5, "08": 145, "09": 230, ...}
```

---

### 2. Package Popularity Tracking (1 day)

**Value:** Know what packages your team installs most

**Database Schema:**
```sql
CREATE TABLE package_usage (
  id TEXT PRIMARY KEY,
  license_id TEXT REFERENCES license(id),
  package_name TEXT NOT NULL,
  source TEXT, -- "pacman", "aur", "npm", "pip", etc.
  install_count INTEGER DEFAULT 0,
  search_count INTEGER DEFAULT 0,
  first_used_at INTEGER,
  last_used_at INTEGER,
  created_at INTEGER,
  updated_at INTEGER,
  UNIQUE(license_id, package_name, source)
);

CREATE INDEX idx_package_usage_license ON package_usage(license_id);
CREATE INDEX idx_package_usage_popular ON package_usage(license_id, install_count DESC);
```

**CLI Payload Update:**
```json
{
  "license_key": "...",
  "machine_id": "...",
  "commands_run": 5,
  "packages": [
    {"name": "ripgrep", "source": "pacman", "action": "installed"},
    {"name": "firefox", "source": "pacman", "action": "searched"}
  ]
}
```

**Dashboard Widget:**
```tsx
<TopPackages>
  <PackageRank rank={1} name="ripgrep" installs={245} trend="+12%" />
  <PackageRank rank={2} name="nodejs" installs={198} trend="+5%" />
  <PackageRank rank={3} name="python" installs={167} trend="-2%" />
</TopPackages>
```

---

### 3. Runtime Version Tracking (1 day)

**Value:** Know what versions teams are using

**Database Schema:**
```sql
CREATE TABLE runtime_usage (
  id TEXT PRIMARY KEY,
  license_id TEXT REFERENCES license(id),
  runtime TEXT NOT NULL, -- "node", "python", "go", "rust", etc.
  version TEXT NOT NULL, -- "20.10.0", "3.12.1", etc.
  machine_count INTEGER DEFAULT 0,
  last_used_at INTEGER,
  created_at INTEGER,
  updated_at INTEGER,
  UNIQUE(license_id, runtime, version)
);

CREATE INDEX idx_runtime_usage_license ON runtime_usage(license_id);
```

**Dashboard Widget:**
```tsx
<RuntimeVersions>
  <RuntimeBar runtime="Node.js" versions={[
    {version: "20.x", usage: 65%, machines: 13},
    {version: "18.x", usage: 30%, machines: 6},
    {version: "16.x", usage: 5%, machines: 1, outdated: true}
  ]} />
</RuntimeVersions>
```

**Alert:** "⚠️ 1 machine still on Node 16 (EOL)"

---

### 4. Command Leaderboard (1 day)

**Value:** Gamification + identify power users

**UI:**
```tsx
<Leaderboard period="this_week">
  <LeaderboardEntry 
    rank={1}
    name="Alice Chen"
    avatar="/avatars/alice.jpg"
    commands={1,247}
    timeSaved="12.5h"
    badge="🏆"
  />
  <LeaderboardEntry 
    rank={2}
    name="Bob Smith"
    commands={892}
    timeSaved="8.2h"
    badge="🥈"
  />
</Leaderboard>
```

**Privacy:** Only show within same team/org

---

### 5. Goal Tracking (2 days)

**Value:** Set and track team goals

**Database Schema:**
```sql
CREATE TABLE goal (
  id TEXT PRIMARY KEY,
  license_id TEXT REFERENCES license(id),
  name TEXT NOT NULL,
  metric TEXT NOT NULL, -- "total_commands", "adoption_rate", "time_saved_ms"
  target_value REAL NOT NULL,
  current_value REAL,
  period TEXT, -- "weekly", "monthly", "quarterly"
  start_date TEXT,
  end_date TEXT,
  status TEXT, -- "in_progress", "achieved", "failed"
  created_at INTEGER,
  updated_at INTEGER
);
```

**Example Goals:**
- "Run 10,000 commands this month" (currently 7,234 → 72%)
- "Achieve 80% team adoption" (currently 65% → on track)
- "Save 100 hours this quarter" (currently 67h → behind)

**UI:**
```tsx
<GoalCard 
  name="10K Commands Challenge"
  progress={72}
  current={7234}
  target={10000}
  daysRemaining={8}
  status="on_track"
  emoji="🎯"
/>
```

---

## Phase 1: Team Foundation (4-6 weeks)

### Database Schema (Complete)

```sql
-- Organizations (top-level entity)
CREATE TABLE organization (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT UNIQUE NOT NULL,
  plan TEXT NOT NULL DEFAULT 'free', -- free, team, enterprise
  max_seats INTEGER DEFAULT 5,
  stripe_customer_id TEXT,
  stripe_subscription_id TEXT,
  billing_email TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX idx_org_slug ON organization(slug);
CREATE INDEX idx_org_stripe ON organization(stripe_customer_id);

-- Teams within organizations
CREATE TABLE team (
  id TEXT PRIMARY KEY,
  organization_id TEXT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  description TEXT,
  max_machines INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(organization_id, slug)
);

CREATE INDEX idx_team_org ON team(organization_id);

-- Organization members (users in org)
CREATE TABLE organization_member (
  id TEXT PRIMARY KEY,
  organization_id TEXT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  role TEXT NOT NULL, -- owner, admin, member, viewer
  invited_by TEXT REFERENCES user(id),
  invited_at INTEGER,
  joined_at INTEGER,
  created_at INTEGER NOT NULL,
  UNIQUE(organization_id, user_id)
);

CREATE INDEX idx_org_member_org ON organization_member(organization_id);
CREATE INDEX idx_org_member_user ON organization_member(user_id);
CREATE INDEX idx_org_member_role ON organization_member(organization_id, role);

-- Team members (users in teams)
CREATE TABLE team_member (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL REFERENCES team(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  role TEXT NOT NULL, -- lead, member
  added_by TEXT REFERENCES user(id),
  added_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(team_id, user_id)
);

CREATE INDEX idx_team_member_team ON team_member(team_id);
CREATE INDEX idx_team_member_user ON team_member(user_id);

-- Invitations (pending team/org joins)
CREATE TABLE invitation (
  id TEXT PRIMARY KEY,
  organization_id TEXT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES team(id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  role TEXT NOT NULL,
  token TEXT UNIQUE NOT NULL,
  invited_by TEXT NOT NULL REFERENCES user(id),
  expires_at INTEGER NOT NULL,
  accepted_at INTEGER,
  created_at INTEGER NOT NULL
);

CREATE INDEX idx_invitation_email ON invitation(email);
CREATE INDEX idx_invitation_token ON invitation(token);
CREATE INDEX idx_invitation_org ON invitation(organization_id);

-- Update existing tables
ALTER TABLE license ADD COLUMN organization_id TEXT REFERENCES organization(id);
ALTER TABLE license ADD COLUMN team_id TEXT REFERENCES team(id);
CREATE INDEX idx_license_org ON license(organization_id);
CREATE INDEX idx_license_team ON license(team_id);

ALTER TABLE machine ADD COLUMN team_id TEXT REFERENCES team(id);
CREATE INDEX idx_machine_team ON machine(team_id);

ALTER TABLE usage_daily ADD COLUMN team_id TEXT REFERENCES team(id);
CREATE INDEX idx_usage_team ON usage_daily(team_id, date);
```

---

### API Endpoints (Complete)

#### Organizations

```typescript
// GET /api/organizations
// List all orgs user is member of
interface OrganizationsResponse {
  organizations: Array<{
    id: string;
    name: string;
    slug: string;
    role: "owner" | "admin" | "member" | "viewer";
    plan: string;
    member_count: number;
    team_count: number;
  }>;
}

// POST /api/organizations
// Create new organization
interface CreateOrganizationRequest {
  name: string;
  slug: string; // auto-generated if not provided
}

// GET /api/organizations/:id
// Get organization details
interface OrganizationResponse {
  id: string;
  name: string;
  slug: string;
  plan: string;
  max_seats: number;
  members: Array<{
    id: string;
    email: string;
    name: string;
    role: string;
    joined_at: string;
  }>;
  teams: Array<{
    id: string;
    name: string;
    member_count: number;
  }>;
  usage: {
    total_commands: number;
    total_time_saved_ms: number;
    active_machines: number;
  };
}

// PATCH /api/organizations/:id
// Update organization
interface UpdateOrganizationRequest {
  name?: string;
  plan?: string;
  max_seats?: number;
}

// DELETE /api/organizations/:id
// Delete organization (owner only)
```

#### Teams

```typescript
// GET /api/organizations/:org_id/teams
// List teams in organization
interface TeamsResponse {
  teams: Array<{
    id: string;
    name: string;
    slug: string;
    member_count: number;
    machine_count: number;
    usage_summary: {
      commands_today: number;
      time_saved_today: number;
    };
  }>;
}

// POST /api/organizations/:org_id/teams
// Create team
interface CreateTeamRequest {
  name: string;
  slug?: string;
  description?: string;
  max_machines?: number;
}

// GET /api/teams/:id
// Get team details
interface TeamResponse {
  id: string;
  organization_id: string;
  name: string;
  description: string;
  members: Array<{
    id: string;
    email: string;
    name: string;
    role: string;
  }>;
  machines: Machine[];
  usage: UsageStats;
  achievements: Achievement[];
}

// PATCH /api/teams/:id
// Update team
interface UpdateTeamRequest {
  name?: string;
  description?: string;
  max_machines?: number;
}

// DELETE /api/teams/:id
// Delete team
```

#### Members

```typescript
// POST /api/organizations/:org_id/invitations
// Invite user to organization
interface InviteRequest {
  email: string;
  role: "admin" | "member" | "viewer";
  team_ids?: string[]; // optional: add to teams
}

// POST /api/invitations/:token/accept
// Accept invitation
interface AcceptInvitationRequest {
  token: string;
}

// DELETE /api/organizations/:org_id/members/:user_id
// Remove member from organization

// POST /api/teams/:team_id/members
// Add member to team
interface AddTeamMemberRequest {
  user_id: string;
  role: "lead" | "member";
}

// DELETE /api/teams/:team_id/members/:user_id
// Remove member from team
```

---

### UI Components

#### 1. Organization Selector

```tsx
<OrganizationSwitcher>
  <OrgOption 
    id="org_abc123"
    name="Acme Corp"
    plan="enterprise"
    active={true}
  />
  <OrgOption 
    id="org_def456"
    name="Startup XYZ"
    plan="team"
  />
  <Divider />
  <CreateOrgButton />
</OrganizationSwitcher>
```

Location: Top nav bar (next to user menu)

#### 2. Team Dashboard

```tsx
<TeamDashboard teamId="team_xyz789">
  <TeamHeader>
    <TeamName>Backend Team</TeamName>
    <TeamStats>
      <Stat label="Members" value={12} />
      <Stat label="Machines" value={24} />
      <Stat label="Commands Today" value={1,247} />
    </TeamStats>
  </TeamHeader>
  
  <TeamMetrics period="last_7_days">
    <MetricCard title="Time Saved" value="42.5h" trend="+15%" />
    <MetricCard title="Commands" value="8,234" trend="+8%" />
    <MetricCard title="Adoption" value="92%" trend="+12%" />
  </TeamMetrics>
  
  <TeamActivity>
    <ActivityChart data={dailyUsage} />
  </TeamActivity>
  
  <TeamMembers>
    <MemberCard user={user} stats={userStats} />
  </TeamMembers>
</TeamDashboard>
```

#### 3. Invitation Flow

```tsx
// Step 1: Invite modal
<InviteMemberModal>
  <EmailInput 
    placeholder="colleague@acme.com"
    validate={isEmail}
  />
  <RoleSelect 
    options={["Admin", "Member", "Viewer"]}
    default="Member"
  />
  <TeamMultiSelect 
    teams={organizationTeams}
    placeholder="Add to teams (optional)"
  />
  <SendInviteButton />
</InviteMemberModal>

// Step 2: Email sent
Subject: You're invited to join Acme Corp on OMG
Body: [Name] invited you to join their team...
CTA: Accept Invitation →

// Step 3: Acceptance page
<InvitationAcceptPage token={token}>
  <OrgInfo name="Acme Corp" memberCount={12} />
  <UserInfo email="you@email.com" role="Member" />
  <AcceptButton /> or <DeclineButton />
</InvitationAcceptPage>
```

#### 4. Team Settings

```tsx
<TeamSettings teamId={teamId}>
  <Section title="General">
    <Input label="Team Name" value={name} />
    <Textarea label="Description" value={description} />
    <Input 
      label="Max Machines" 
      type="number" 
      value={maxMachines} 
    />
  </Section>
  
  <Section title="Members">
    <MemberList>
      {members.map(m => (
        <MemberRow 
          user={m} 
          role={m.role}
          onRemove={() => removeMember(m.id)}
          onChangeRole={(role) => updateRole(m.id, role)}
        />
      ))}
    </MemberList>
    <AddMemberButton />
  </Section>
  
  <Section title="Danger Zone">
    <DeleteTeamButton />
  </Section>
</TeamSettings>
```

---

## Implementation Order

### Week 1-2: Database & API Foundation
- [ ] Create migration scripts
- [ ] Implement organization CRUD
- [ ] Implement team CRUD
- [ ] Add org_id/team_id to existing tables
- [ ] Data migration script (existing users → personal orgs)

### Week 3-4: Invitation System
- [ ] Invitation table and logic
- [ ] Email sending (SMTP integration)
- [ ] Invitation acceptance flow
- [ ] Member management APIs

### Week 5-6: UI Implementation
- [ ] Organization switcher in nav
- [ ] Team dashboard page
- [ ] Invitation modal
- [ ] Team settings page
- [ ] Member management UI

### Week 7-8: Testing & Polish
- [ ] E2E tests for invitation flow
- [ ] Permission tests (RBAC)
- [ ] Performance testing (100+ teams)
- [ ] UI polish and animations
- [ ] Documentation

---

## RBAC Permission Matrix

| Action | Owner | Admin | Member | Viewer |
|--------|-------|-------|--------|--------|
| **Organization** | | | | |
| View org details | ✅ | ✅ | ✅ | ✅ |
| Update org settings | ✅ | ❌ | ❌ | ❌ |
| Delete org | ✅ | ❌ | ❌ | ❌ |
| View billing | ✅ | ✅ | ❌ | ❌ |
| **Teams** | | | | |
| Create team | ✅ | ✅ | ❌ | ❌ |
| Update team | ✅ | ✅ | Lead | ❌ |
| Delete team | ✅ | ✅ | ❌ | ❌ |
| **Members** | | | | |
| Invite member | ✅ | ✅ | ❌ | ❌ |
| Remove member | ✅ | ✅ | ❌ | ❌ |
| Change roles | ✅ | Admin→Member | ❌ | ❌ |
| **Data** | | | | |
| View team data | ✅ | ✅ | ✅ | ✅ |
| Export data | ✅ | ✅ | ✅ | ❌ |
| Delete data | ✅ | ❌ | ❌ | ❌ |

**Implementation:**
```typescript
function canPerformAction(
  user: User, 
  org: Organization, 
  action: string
): boolean {
  const member = org.members.find(m => m.userId === user.id);
  if (!member) return false;
  
  const permissions = PERMISSION_MATRIX[member.role];
  return permissions.includes(action);
}

// Usage
if (!canPerformAction(user, org, 'team.create')) {
  throw new Error('Insufficient permissions');
}
```

---

## Migration Strategy

### Existing Users → Personal Organizations

Every existing user gets a personal organization:

```typescript
async function migrateUsersToOrgs(db: D1Database) {
  const users = await db.select().from(user).all();
  
  for (const user of users) {
    const orgId = `org_${user.id}`;
    const orgSlug = user.email.split('@')[0]; // "john" from "john@example.com"
    
    // Create personal organization
    await db.insert(organization).values({
      id: orgId,
      name: `${user.name}'s Organization`,
      slug: orgSlug,
      plan: 'free',
      max_seats: 1,
      created_at: Date.now(),
      updated_at: Date.now(),
    });
    
    // Add user as owner
    await db.insert(organizationMember).values({
      id: `om_${user.id}`,
      organization_id: orgId,
      user_id: user.id,
      role: 'owner',
      joined_at: Date.now(),
      created_at: Date.now(),
    });
    
    // Create default team
    const teamId = `team_default_${user.id}`;
    await db.insert(team).values({
      id: teamId,
      organization_id: orgId,
      name: 'Personal',
      slug: 'personal',
      created_at: Date.now(),
      updated_at: Date.now(),
    });
    
    // Add user to team
    await db.insert(teamMember).values({
      id: `tm_${user.id}`,
      team_id: teamId,
      user_id: user.id,
      role: 'lead',
      added_at: Date.now(),
      created_at: Date.now(),
    });
    
    // Link existing licenses to org
    await db.update(license)
      .set({ 
        organization_id: orgId,
        team_id: teamId 
      })
      .where(eq(license.userId, user.id));
  }
}
```

**Rollback Plan:**
Keep backup SQL before migration. If fails, restore from backup.

---

## Testing Strategy

### Unit Tests
```typescript
describe('Organization API', () => {
  test('creates organization with valid data', async () => {
    const response = await POST('/api/organizations', {
      name: 'Test Corp',
      slug: 'test-corp'
    });
    expect(response.status).toBe(201);
    expect(response.data.slug).toBe('test-corp');
  });
  
  test('rejects duplicate slug', async () => {
    await POST('/api/organizations', { name: 'A', slug: 'test' });
    const response = await POST('/api/organizations', { name: 'B', slug: 'test' });
    expect(response.status).toBe(409); // Conflict
  });
});
```

### Integration Tests
```typescript
describe('Invitation Flow', () => {
  test('complete invitation workflow', async () => {
    // Create org
    const org = await createOrg({ name: 'Acme' });
    
    // Invite user
    const invitation = await POST(`/api/organizations/${org.id}/invitations`, {
      email: 'new@user.com',
      role: 'member'
    });
    
    // Accept invitation
    const response = await POST(`/api/invitations/${invitation.token}/accept`);
    expect(response.status).toBe(200);
    
    // Verify membership
    const members = await GET(`/api/organizations/${org.id}/members`);
    expect(members).toContainEqual(
      expect.objectContaining({ email: 'new@user.com' })
    );
  });
});
```

### E2E Tests (Playwright)
```typescript
test('user can create team and invite members', async ({ page }) => {
  await page.goto('/dashboard');
  
  // Create organization
  await page.click('[data-testid="create-org"]');
  await page.fill('[name="name"]', 'Test Org');
  await page.click('[type="submit"]');
  
  // Create team
  await page.click('[data-testid="create-team"]');
  await page.fill('[name="name"]', 'Engineering');
  await page.click('[type="submit"]');
  
  // Invite member
  await page.click('[data-testid="invite-member"]');
  await page.fill('[name="email"]', 'colleague@example.com');
  await page.selectOption('[name="role"]', 'member');
  await page.click('[type="submit"]');
  
  // Verify invitation sent
  await expect(page.locator('text=Invitation sent')).toBeVisible();
});
```

---

## Performance Considerations

### Query Optimization

**Bad:**
```sql
-- N+1 query problem
SELECT * FROM team WHERE organization_id = ?;
-- Then for each team:
SELECT COUNT(*) FROM team_member WHERE team_id = ?;
```

**Good:**
```sql
-- Single query with JOIN
SELECT 
  t.*,
  COUNT(tm.id) as member_count
FROM team t
LEFT JOIN team_member tm ON t.id = tm.team_id
WHERE t.organization_id = ?
GROUP BY t.id;
```

### Caching Strategy

```typescript
// Cache org data for 5 minutes
const getCachedOrg = async (orgId: string) => {
  const cacheKey = `org:${orgId}`;
  const cached = await env.CACHE.get(cacheKey);
  
  if (cached) {
    return JSON.parse(cached);
  }
  
  const org = await fetchOrgFromDB(orgId);
  await env.CACHE.put(cacheKey, JSON.stringify(org), {
    expirationTtl: 300 // 5 minutes
  });
  
  return org;
};
```

### Database Indexes

**Critical indexes for team queries:**
```sql
CREATE INDEX idx_team_org_id ON team(organization_id);
CREATE INDEX idx_team_member_team ON team_member(team_id);
CREATE INDEX idx_team_member_user ON team_member(user_id);
CREATE INDEX idx_usage_team_date ON usage_daily(team_id, date DESC);
```

---

## Launch Checklist

### Pre-Launch
- [ ] Database migrations tested on staging
- [ ] API endpoints tested (Postman collection)
- [ ] UI components reviewed (design approval)
- [ ] Permission logic tested (security review)
- [ ] Performance tested (100+ orgs, 1000+ teams)
- [ ] Email templates designed
- [ ] Documentation written
- [ ] Changelog prepared

### Launch Day
- [ ] Run migrations on production
- [ ] Deploy new code
- [ ] Monitor error rates (Sentry)
- [ ] Monitor performance (Cloudflare Analytics)
- [ ] Send announcement email to existing users
- [ ] Post on Twitter/LinkedIn
- [ ] Update pricing page

### Post-Launch (Week 1)
- [ ] Gather user feedback
- [ ] Fix critical bugs
- [ ] Monitor adoption rates
- [ ] Iterate on UX issues
- [ ] Plan Phase 2 features

---

## Success Metrics

### Adoption (30 days post-launch)
- 25% of existing users create organizations
- 10% invite team members
- 50+ teams created
- 200+ invitations sent

### Engagement (60 days)
- 40% of orgs have 2+ teams
- 30% of orgs have 3+ members
- 70% weekly active rate for team features

### Revenue (90 days)
- 5 teams upgrade to paid ($199/mo)
- $1,000 MRR from team features
- 3 enterprise inquiries

---

## Next: Implement Quick Wins This Week

Start with the low-hanging fruit:
1. Activity heatmap (2 days)
2. Package tracking (1 day)
3. Runtime versions (1 day)
4. Command leaderboard (1 day)

These provide immediate value while planning Phase 1 (Teams).

**Total time:** 5 days → High-impact features shipping this week!
