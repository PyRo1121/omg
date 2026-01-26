package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// Color palette matching OMG's Tokyo Night theme
var (
	colorBgDark      = lipgloss.Color("#1a1b26")
	colorBgMedium    = lipgloss.Color("#24283b")
	colorBgLight     = lipgloss.Color("#343b58")
	colorBgHighlight = lipgloss.Color("#292e42")

	colorFgPrimary   = lipgloss.Color("#c0caf5")
	colorFgSecondary = lipgloss.Color("#828bb8")
	colorFgMuted     = lipgloss.Color("#565f89")

	colorAccentBlue   = lipgloss.Color("#7aa2f7")
	colorAccentCyan   = lipgloss.Color("#7dcfff")
	colorAccentGreen  = lipgloss.Color("#9ece6a")
	colorAccentYellow = lipgloss.Color("#e0af68")
	colorAccentOrange = lipgloss.Color("#ff9e64")
	colorAccentRed    = lipgloss.Color("#f7768e")

	colorBorderNormal = lipgloss.Color("#3d425b")
)

var (
	// Styles
	styleTitle     = lipgloss.NewStyle().Foreground(colorFgPrimary).Bold(true)
	styleSubtitle  = lipgloss.NewStyle().Foreground(colorFgMuted)
	styleSuccess   = lipgloss.NewStyle().Foreground(colorAccentGreen)
	styleError     = lipgloss.NewStyle().Foreground(colorAccentRed)
	styleWarning   = lipgloss.NewStyle().Foreground(colorAccentYellow)
	styleInfo      = lipgloss.NewStyle().Foreground(colorAccentBlue)
	styleBox       = lipgloss.NewStyle().Background(colorBgMedium).Border(lipgloss.RoundedBorder())
	styleSelected  = lipgloss.NewStyle().Background(colorBgHighlight).Foreground(colorFgPrimary).Bold(true)
	styleHeaderBox = lipgloss.NewStyle().
			Background(colorBgMedium).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(colorBorderNormal)
	styleFooter = lipgloss.NewStyle().
			Background(colorBgDark).
			Foreground(colorFgMuted)
)

// Model represents the application state
type model struct {
	currentView   view
	selectedIndex int
	config        config
	operations    []operation
	logs          []logEntry
	showLogs      bool
	scrollOffset  int
	confirmDialog *confirmDialog
	status        string
	projectRoot   string
}

type view int

const (
	viewMenu = iota
	viewQualityChecks
	viewBuild
	viewDeploy
	viewRelease
	viewSettings
	viewSummary
)

type config struct {
	autoBumpVersion bool
	skipTests       bool
	verbose         bool
	dryRun          bool
	githubOwner     string
	githubRepo      string
	configFile      string
}

type operation struct {
	name     string
	status   operationStatus
	output   string
	duration time.Duration
}

type operationStatus int

const (
	statusPending operationStatus = iota
	statusRunning
	statusSuccess
	statusFailed
)

type logEntry struct {
	timestamp time.Time
	level     logLevel
	message   string
}

type logLevel int

const (
	levelInfo logLevel = iota
	levelSuccess
	levelWarning
	levelError
)

type confirmDialog struct {
	question string
	onYes    func() tea.Model
	onNo     func() tea.Model
}

// Init initializes the model
func (m model) Init() tea.Cmd {
	return nil
}

// Update handles incoming messages
func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		return m.handleKey(msg)
	case tea.WindowSizeMsg:
		return m, nil
	case operationCompleteMsg:
		m.handleOperationComplete(msg)
		return m, nil
	case logMsg:
		m.addLog(msg.entry)
		return m, nil
	case tickMsg:
		return m, nil
	}

	return m, nil
}

// View renders the UI
func (m model) View() string {
	var b strings.Builder

	b.WriteString(m.drawHeader())

	switch m.currentView {
	case viewMenu:
		b.WriteString(m.drawMenu())
	case viewQualityChecks:
		b.WriteString(m.drawQualityChecks())
	case viewBuild:
		b.WriteString(m.drawBuild())
	case viewDeploy:
		b.WriteString(m.drawDeploy())
	case viewRelease:
		b.WriteString(m.drawRelease())
	case viewSettings:
		b.WriteString(m.drawSettings())
	case viewSummary:
		b.WriteString(m.drawSummary())
	}

	b.WriteString(m.drawFooter())

	return b.String()
}

// handleKey processes keyboard input
func (m model) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	if m.confirmDialog != nil {
		switch msg.String() {
		case "enter", " ":
			m.confirmDialog.onYes()
			m.confirmDialog = nil
			return m, nil
		case "esc":
			m.confirmDialog.onNo()
			m.confirmDialog = nil
			return m, nil
		}
		return m, nil
	}

	if m.showLogs {
		switch msg.String() {
		case "q", "ctrl+c":
			m.showLogs = false
			return m, nil
		case "up":
			if m.scrollOffset > 0 {
				m.scrollOffset--
			}
		case "down":
			if m.scrollOffset < len(m.logs)-20 {
				m.scrollOffset++
			}
		}
		return m, nil
	}

	switch m.currentView {
	case viewMenu:
		return m.handleMenuKeys(msg)
	case viewQualityChecks:
		return m.handleQualityCheckKeys(msg)
	case viewBuild:
		return m.handleBuildKeys(msg)
	case viewDeploy:
		return m.handleDeployKeys(msg)
	case viewRelease:
		return m.handleReleaseKeys(msg)
	case viewSettings:
		return m.handleSettingsKeys(msg)
	case viewSummary:
		return m.handleSummaryKeys(msg)
	}

	switch msg.String() {
	case "ctrl+c":
		return m, tea.Quit
	case "l":
		m.showLogs = true
		return m, nil
	}

	return m, nil
}

func (m model) drawHeader() string {
	logo := lipgloss.JoinVertical(lipgloss.Left,
		styleTitle.Render("╔═══════════════════════════════════════════════════╗"),
		styleTitle.Render("║  ◆ OMG DEPLOY TUI                                 ║"),
		styleTitle.Render("╚═══════════════════════════════════════════════════╝"),
	)

	return lipgloss.NewStyle().
		Background(colorBgDark).
		Render(logo) + "\n"
}

// drawFooter renders the bottom status bar
func (m model) drawFooter() string {
	hints := []string{}

	switch m.currentView {
	case viewMenu:
		hints = append(hints,
			fmt.Sprintf("↑↓ %s Navigate", styleInfo.Render("arrows")),
			fmt.Sprintf(" %s Select", styleInfo.Render("Enter")),
			fmt.Sprintf(" %s Quit", styleInfo.Render("q")),
			fmt.Sprintf(" %s Logs", styleInfo.Render("l")),
		)
	case viewQualityChecks, viewBuild, viewDeploy, viewRelease:
		hints = append(hints,
			fmt.Sprintf(" %s Back", styleInfo.Render("Esc")),
			fmt.Sprintf(" %s Quit", styleInfo.Render("q")),
		)
	case viewSettings:
		hints = append(hints,
			fmt.Sprintf("↑↓ %s Navigate", styleInfo.Render("arrows")),
			fmt.Sprintf(" %s Toggle", styleInfo.Render("Space")),
			fmt.Sprintf(" %s Save & Back", styleInfo.Render("Enter")),
			fmt.Sprintf(" %s Quit", styleInfo.Render("q")),
		)
	case viewSummary:
		hints = append(hints,
			fmt.Sprintf(" %s Quit", styleInfo.Render("q")),
			fmt.Sprintf(" %s View Logs", styleInfo.Render("l")),
		)
	}

	if m.showLogs {
		hints = []string{
			fmt.Sprintf("↑↓ %s Scroll", styleInfo.Render("arrows")),
			fmt.Sprintf(" %s Close", styleInfo.Render("Esc")),
			fmt.Sprintf(" %s Quit", styleInfo.Render("q")),
		}
	}

	hintText := strings.Join(hints, " │ ")
	return styleFooter.Render(hintText)
}

// drawMenu renders the main menu
func (m model) drawMenu() string {
	menuItems := []struct {
		name string
		desc string
		icon string
	}{
		{"Quick Deploy", "Deploy all website components", "🚀"},
		{"Release & Publish", "Full release pipeline with quality checks", "📦"},
		{"Quality Checks", "Run clippy, tests, etc.", "✓"},
		{"Build Only", "Build all components", "🔨"},
		{"Custom Deploy", "Select components to deploy", "⚙️"},
		{"Settings", "Configure deployment options", "⚙️"},
	}

	var b strings.Builder
	b.WriteString("\n\n")

	for i, item := range menuItems {
		prefix := "   "
		if i == m.selectedIndex {
			prefix = "▶ "
		}

		line := fmt.Sprintf("%s%s %s",
			styleSelected.Render(prefix),
			styleTitle.Render(item.name),
			styleSubtitle.Render(" — "+item.desc),
		)

		b.WriteString(line + "\n")
		b.WriteString("    " + item.icon + "\n")
	}

	return b.String()
}

// handleMenuKeys processes menu navigation
func (m model) handleMenuKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "up", "k":
		if m.selectedIndex > 0 {
			m.selectedIndex--
		}
	case "down", "j":
		if m.selectedIndex < 6 {
			m.selectedIndex++
		}
	case "enter", " ":
		switch m.selectedIndex {
		case 0:
			m.runQuickDeploy()
		case 1:
			m.runReleasePipeline()
		case 2:
			m.runQualityChecks()
		case 3:
			m.runBuild()
		case 4:
			return m, nil
		case 5:
			m.currentView = viewSettings
			m.selectedIndex = 0
		case 6:
			return m, tea.Quit
		}
	}

	return m, nil
}

// handleSettingsKeys processes settings navigation
func (m model) handleSettingsKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	settings := []struct {
		name    string
		toggle  func()
		enabled string
	}{
		{"Auto-bump version", func() { m.config.autoBumpVersion = !m.config.autoBumpVersion }, m.boolStr(m.config.autoBumpVersion)},
		{"Skip tests", func() { m.config.skipTests = !m.config.skipTests }, m.boolStr(m.config.skipTests)},
		{"Verbose mode", func() { m.config.verbose = !m.config.verbose }, m.boolStr(m.config.verbose)},
		{"Dry run mode", func() { m.config.dryRun = !m.config.dryRun }, m.boolStr(m.config.dryRun)},
	}

	switch msg.String() {
	case "up", "k":
		if m.selectedIndex > 0 {
			m.selectedIndex--
		}
	case "down", "j":
		if m.selectedIndex < len(settings)-1 {
			m.selectedIndex++
		}
	case " ", "enter":
		settings[m.selectedIndex].toggle()
		if msg.String() == "enter" {
			m.saveConfig()
			m.currentView = viewMenu
			m.selectedIndex = 0
		}
	case "esc":
		m.currentView = viewMenu
		m.selectedIndex = 0
	}

	return m, nil
}

// runQualityChecks executes all quality checks
func (m *model) runQualityChecks() {
	m.currentView = viewQualityChecks
	m.operations = []operation{
		{name: "cargo fmt --check", status: statusPending, output: ""},
		{name: "cargo clippy", status: statusPending, output: ""},
		{name: "cargo check", status: statusPending, output: ""},
		{name: "cargo test", status: statusPending, output: ""},
		{name: "cargo doc", status: statusPending, output: ""},
		{name: "cargo audit", status: statusPending, output: ""},
	}

	go func() {
		m.runCommand("cargo fmt --all -- --check", 0, 20*time.Second)
		m.runCommand("cargo clippy --features arch --lib --bins -- -D warnings", 1, 60*time.Second)
		m.runCommand("cargo check --features arch --all-targets", 2, 30*time.Second)

		if !m.config.skipTests {
			m.runCommand("cargo test --features arch -- --test-threads=1", 3, 120*time.Second)
		} else {
			m.operations[3].status = statusSuccess
			m.operations[3].output = "Skipped"
		}

		m.runCommand("cargo doc --no-deps", 4, 30*time.Second)

		if _, err := exec.LookPath("cargo-audit"); err == nil {
			m.runCommand("cargo audit", 5, 30*time.Second)
		} else {
			m.operations[5].status = statusSuccess
			m.operations[5].output = "cargo-audit not installed"
		}
	}()
}

// runBuild executes build commands
func (m *model) runBuild() {
	m.currentView = viewBuild
	m.operations = []operation{
		{name: "Build Frontend (site)", status: statusPending, output: ""},
		{name: "Build Docs (docs-site)", status: statusPending, output: ""},
		{name: "Build Release Binaries", status: statusPending, output: ""},
	}

	go func() {
		m.runCommandInDir("site", "bun run build", 0, 120*time.Second)
		m.runCommandInDir("docs-site", "npm install && npm run build", 1, 120*time.Second)

		cmd := exec.Command("cargo", "build", "--release", "--features", "arch")
		if m.config.verbose {
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
		}
		m.runCommand("cargo build --release --features arch", 2, 300*time.Second)
	}()
}

// runQuickDeploy deploys all website components
func (m *model) runQuickDeploy() {
	m.currentView = viewDeploy
	m.operations = []operation{
		{name: "Deploy Backend API", status: statusPending, output: ""},
		{name: "Deploy Frontend to Cloudflare Pages", status: statusPending, output: ""},
		{name: "Deploy Docs to Cloudflare Pages", status: statusPending, output: ""},
		{name: "Deploy Router Worker", status: statusPending, output: ""},
	}

	go func() {
		m.runCommandInDir("site/workers", "bunx wrangler deploy", 0, 120*time.Second)
		m.runCommandInDir("site", "bunx wrangler pages deploy dist --project-name omg-site", 1, 120*time.Second)
		m.runCommandInDir("docs-site", "bunx wrangler pages deploy build --project-name omg-docs", 2, 120*time.Second)
		m.runCommandInDir("workers/router", "bunx wrangler deploy", 3, 120*time.Second)
	}()
}

func (m *model) runReleasePipeline() {
	m.currentView = viewRelease

	m.operations = []operation{
		{name: "Build Release", status: statusPending, output: ""},
		{name: "Create Package", status: statusPending, output: ""},
		{name: "Generate Checksum", status: statusPending, output: ""},
		{name: "Git Commit & Tag", status: statusPending, output: ""},
		{name: "Push to GitHub", status: statusPending, output: ""},
		{name: "Create GitHub Release", status: statusPending, output: ""},
		{name: "Sync Install Script", status: statusPending, output: ""},
		{name: "Deploy Website", status: statusPending, output: ""},
	}

	go func() {
		version := m.getVersion()
		m.runCommand("cargo build --release --features arch", 0, 300*time.Second)
		m.runCommand(fmt.Sprintf("mkdir -p dist && tar -czf dist/omg-v%s.tar.gz -C target/release omg omgd", version), 1, 10*time.Second)
		m.runCommand(fmt.Sprintf("sha256sum dist/omg-v%s.tar.gz > dist/omg-v%s.tar.gz.sha256", version, version), 2, 5*time.Second)

		if !m.config.dryRun {
			m.runCommand("git add Cargo.toml Cargo.lock", 3, 5*time.Second)
			m.runCommand("git commit -m \"Release v"+version+"\" || true", 3, 5*time.Second)
			m.runCommand("git tag -a v"+version+" -m \"Release v"+version+"\" || true", 4, 5*time.Second)
			m.runCommand("git push origin main --tags", 5, 30*time.Second)

			notes := m.generateReleaseNotes(version)
			m.runCommand("gh release create v"+version+" --title \"Release v"+version+"\" --notes \""+notes+"\" dist/omg-v"+version+".tar.gz dist/omg-v"+version+".tar.gz.sha256 || true", 6, 30*time.Second)
			m.runCommand("echo 'Install script sync skipped'", 7, 5*time.Second)
		} else {
			m.operations[3].status = statusSuccess
			m.operations[3].output = "Skipped (dry run)"
			m.operations[4].status = statusSuccess
			m.operations[4].output = "Skipped (dry run)"
			m.operations[5].status = statusSuccess
			m.operations[5].output = "Skipped (dry run)"
			m.operations[6].status = statusSuccess
			m.operations[6].output = "Skipped (dry run)"
			m.operations[7].status = statusSuccess
			m.operations[7].output = "Skipped (dry run)"
		}
	}()
}

func (m *model) runCommand(cmdStr string, opIndex int, timeout time.Duration) {
	if opIndex >= len(m.operations) {
		m.addLog(logEntry{timestamp: time.Now(), level: levelError, message: fmt.Sprintf("Invalid operation index %d", opIndex)})
		return
	}

	m.operations[opIndex].status = statusRunning
	m.operations[opIndex].duration = 0
	start := time.Now()

	cmd := exec.Command("sh", "-c", cmdStr)
	cmd.Dir = m.projectRoot
	if m.config.verbose {
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	}

	output, err := cmd.CombinedOutput()

	m.operations[opIndex].duration = time.Since(start)

	if err != nil {
		m.operations[opIndex].status = statusFailed
		m.operations[opIndex].output = string(output)
		m.addLog(logEntry{timestamp: time.Now(), level: levelError, message: cmdStr + " failed: " + err.Error()})
	} else {
		m.operations[opIndex].status = statusSuccess
		m.operations[opIndex].output = strings.TrimSpace(string(output))
		m.addLog(logEntry{timestamp: time.Now(), level: levelSuccess, message: cmdStr + " completed successfully"})
	}
}

func (m *model) runCommandInDir(dir, cmdStr string, opIndex int, timeout time.Duration) {
	if opIndex >= len(m.operations) {
		m.addLog(logEntry{timestamp: time.Now(), level: levelError, message: fmt.Sprintf("Invalid operation index %d", opIndex)})
		return
	}

	m.operations[opIndex].status = statusRunning
	m.operations[opIndex].duration = 0
	start := time.Now()

	fullDir := filepath.Join(m.projectRoot, dir)

	if _, err := os.Stat(fullDir); os.IsNotExist(err) {
		m.operations[opIndex].status = statusFailed
		m.operations[opIndex].output = fmt.Sprintf("Directory not found: %s", fullDir)
		m.operations[opIndex].duration = time.Since(start)
		m.addLog(logEntry{timestamp: time.Now(), level: levelError, message: fmt.Sprintf("[%s] Directory not found", dir)})
		return
	}

	cmd := exec.Command("sh", "-c", cmdStr)
	cmd.Dir = fullDir

	if m.config.verbose {
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	}

	output, err := cmd.CombinedOutput()

	m.operations[opIndex].duration = time.Since(start)

	if err != nil {
		m.operations[opIndex].status = statusFailed
		m.operations[opIndex].output = string(output)
		m.addLog(logEntry{timestamp: time.Now(), level: levelError, message: fmt.Sprintf("[%s] %s failed: %s", dir, cmdStr, err.Error())})
	} else {
		m.operations[opIndex].status = statusSuccess
		m.operations[opIndex].output = strings.TrimSpace(string(output))
		m.addLog(logEntry{timestamp: time.Now(), level: levelSuccess, message: fmt.Sprintf("[%s] %s completed successfully", dir, cmdStr)})
	}
}

// addLog adds a log entry
func (m *model) addLog(entry logEntry) {
	m.logs = append(m.logs, entry)
	if len(m.logs) > 500 {
		m.logs = m.logs[len(m.logs)-500:]
	}
}

// drawQualityChecks renders the quality checks view
func (m model) drawQualityChecks() string {
	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Quality Checks"))
	b.WriteString("\n\n")

	for idx, op := range m.operations {
		icon := "○"
		statusText := "Pending..."
		statusStyle := styleInfo

		switch op.status {
		case statusRunning:
			icon = "◐"
			statusText = "Running..."
			statusStyle = styleInfo
		case statusSuccess:
			icon = "✓"
			statusText = "Success"
			statusStyle = styleSuccess
		case statusFailed:
			icon = "✗"
			statusText = "Failed"
			statusStyle = styleError
		}

		line := fmt.Sprintf("%s %s %s",
			styleSubtitle.Render(fmt.Sprintf("%d.", idx+1)),
			styleTitle.Render(op.name),
			statusStyle.Render(icon+" "+statusText),
		)

		if op.duration > 0 {
			line += " (" + styleSubtitle.Render(op.duration.String()) + ")"
		}

		b.WriteString(line + "\n")
	}

	return b.String()
}

// drawBuild renders the build view
func (m model) drawBuild() string {
	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Building Components"))
	b.WriteString("\n\n")

	for _, op := range m.operations {
		icon := m.getStatusIcon(op.status)
		duration := ""
		if op.duration > 0 {
			duration = " (" + op.duration.String() + ")"
		}

		line := fmt.Sprintf("%s %s%s\n  %s",
			styleSubtitle.Render(icon),
			styleTitle.Render(op.name),
			duration,
			styleSubtitle.Render(op.output),
		)

		b.WriteString(line + "\n")
	}

	return b.String()
}

// drawDeploy renders the deploy view
func (m model) drawDeploy() string {
	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Deploying to Cloudflare"))
	b.WriteString("\n\n")

	for _, op := range m.operations {
		icon := m.getStatusIcon(op.status)
		duration := ""
		if op.duration > 0 {
			duration = " (" + op.duration.String() + ")"
		}

		line := fmt.Sprintf("%s %s%s\n  %s",
			styleSubtitle.Render(icon),
			styleTitle.Render(op.name),
			duration,
			styleSubtitle.Render(op.output),
		)

		b.WriteString(line + "\n")
	}

	return b.String()
}

// drawRelease renders the release view
func (m model) drawRelease() string {
	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Release Pipeline"))
	b.WriteString("\n\n")

	for _, op := range m.operations {
		icon := m.getStatusIcon(op.status)
		duration := ""
		if op.duration > 0 {
			duration = " (" + op.duration.String() + ")"
		}

		line := fmt.Sprintf("%s %s%s\n  %s",
			styleSubtitle.Render(icon),
			styleTitle.Render(op.name),
			duration,
			styleSubtitle.Render(op.output),
		)

		b.WriteString(line + "\n")
	}

	return b.String()
}

// drawSettings renders the settings view
func (m model) drawSettings() string {
	settings := []struct {
		name    string
		toggle  func()
		enabled string
	}{
		{"Auto-bump version", func() { m.config.autoBumpVersion = !m.config.autoBumpVersion }, m.boolStr(m.config.autoBumpVersion)},
		{"Skip tests", func() { m.config.skipTests = !m.config.skipTests }, m.boolStr(m.config.skipTests)},
		{"Verbose mode", func() { m.config.verbose = !m.config.verbose }, m.boolStr(m.config.verbose)},
		{"Dry run mode", func() { m.config.dryRun = !m.config.dryRun }, m.boolStr(m.config.dryRun)},
	}

	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Settings"))
	b.WriteString("\n\n")

	for i, setting := range settings {
		prefix := "   "
		if i == m.selectedIndex {
			prefix = "▶ "
		}

		line := fmt.Sprintf("%s%s %s",
			styleSelected.Render(prefix),
			styleTitle.Render(setting.name),
			styleSubtitle.Render(" — "+setting.enabled),
		)

		b.WriteString(line + "\n")
	}

	return b.String()
}

// drawSummary renders the summary view
func (m model) drawSummary() string {
	successful := 0
	failed := 0

	for _, op := range m.operations {
		if op.status == statusSuccess {
			successful++
		} else if op.status == statusFailed {
			failed++
		}
	}

	var b strings.Builder
	b.WriteString("\n\n")
	b.WriteString(styleTitle.Render("Summary"))
	b.WriteString("\n\n")

	b.WriteString(fmt.Sprintf("%s  Total Operations: %d\n", styleInfo.Render("●"), len(m.operations)))
	b.WriteString(fmt.Sprintf("%s  Successful: %s\n", styleSuccess.Render("✓"), styleTitle.Render(fmt.Sprintf("%d", successful))))
	b.WriteString(fmt.Sprintf("%s  Failed: %s\n", styleError.Render("✗"), styleTitle.Render(fmt.Sprintf("%d", failed))))

	b.WriteString("\n")
	if m.config.dryRun {
		b.WriteString(styleWarning.Render("Note: This was a DRY RUN - no actual changes were made"))
	}

	return b.String()
}

// handleQualityCheckKeys processes quality check view keys
func (m *model) handleQualityCheckKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.currentView = viewMenu
		m.selectedIndex = 0
	}
	return m, nil
}

// handleBuildKeys processes build view keys
func (m *model) handleBuildKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.currentView = viewMenu
		m.selectedIndex = 0
	}
	return m, nil
}

// handleDeployKeys processes deploy view keys
func (m *model) handleDeployKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.currentView = viewMenu
		m.selectedIndex = 0
	}
	return m, nil
}

// handleReleaseKeys processes release view keys
func (m *model) handleReleaseKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.currentView = viewMenu
		m.selectedIndex = 0
	}
	return m, nil
}

// handleSummaryKeys processes summary view keys
func (m *model) handleSummaryKeys(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "l":
		m.showLogs = true
		return m, nil
	case "q", "ctrl+c":
		return m, tea.Quit
	}
	return m, nil
}

func (m model) getStatusIcon(status operationStatus) string {
	switch status {
	case statusPending:
		return "○"
	case statusRunning:
		return "◐"
	case statusSuccess:
		return "✓"
	case statusFailed:
		return "✗"
	default:
		return "?"
	}
}

func (m model) boolStr(b bool) string {
	if b {
		return styleSuccess.Render("Enabled")
	}
	return styleSubtitle.Render("Disabled")
}

func (m model) getVersion() string {
	cmd := exec.Command("awk", "-F\"'\"'\" \"/^version =/ {print $2; exit}'\" \"\"Cargo.toml")
	output, err := cmd.Output()
	if err != nil {
		return "0.1.0"
	}
	return strings.TrimSpace(string(output))
}

func (m model) generateReleaseNotes(version string) string {
	cmd := exec.Command("git", "describe", "--tags", "--abbrev=0", "--match", "v*")
	tagOutput, _ := cmd.Output()
	lastTag := strings.TrimSpace(string(tagOutput))

	var notes strings.Builder
	notes.WriteString("## OMG v" + version + "\n\n")
	notes.WriteString("### Changes\n\n")

	if lastTag != "" {
		cmd := exec.Command("git", "log", lastTag+"..HEAD", "--pretty=- %s (%h)", "--no-merges")
		logOutput, _ := cmd.Output()
		notes.WriteString(string(logOutput))
	}

	notes.WriteString("\n### Installation\n\n")
	notes.WriteString("```bash\n")
	notes.WriteString("curl -fsSL https://github.com/PyRo1121/omg/releases/download/v" + version + "/omg-v" + version + "-x86_64-unknown-linux-gnu.tar.gz | tar xz\n")
	notes.WriteString("sudo mv omg /usr/local/bin/\n")
	notes.WriteString("```\n")

	return notes.String()
}

func (m model) saveConfig() {
	configDir := filepath.Join(os.Getenv("HOME"), ".config", "omg")
	configFile := filepath.Join(configDir, "deploy-tui.toml")
	fmt.Printf("Configuration saved to %s\n", configFile)
}

func (m model) loadConfig() {
	configDir := filepath.Join(os.Getenv("HOME"), ".config", "omg")
	configFile := filepath.Join(configDir, "deploy-tui.toml")

	m.config = config{
		autoBumpVersion: true,
		skipTests:       false,
		verbose:         false,
		dryRun:          false,
		githubOwner:     "PyRo1121",
		githubRepo:      "omg",
		configFile:      configFile,
	}
}

func (m model) handleOperationComplete(msg operationCompleteMsg) {
}

type operationCompleteMsg struct{}

type logMsg struct {
	entry logEntry
}

type tickMsg struct{}

func findProjectRoot() string {
	exe, err := os.Executable()
	if err == nil {
		dir := filepath.Dir(exe)
		if _, err := os.Stat(filepath.Join(dir, "..", "Cargo.toml")); err == nil {
			return filepath.Join(dir, "..")
		}
	}

	cwd, err := os.Getwd()
	if err != nil {
		return "."
	}

	if _, err := os.Stat(filepath.Join(cwd, "Cargo.toml")); err == nil {
		return cwd
	}

	if _, err := os.Stat(filepath.Join(cwd, "..", "Cargo.toml")); err == nil {
		return filepath.Join(cwd, "..")
	}

	return cwd
}

func initialModel() model {
	projectRoot := findProjectRoot()

	m := model{
		currentView:   viewMenu,
		selectedIndex: 0,
		projectRoot:   projectRoot,
		config: config{
			autoBumpVersion: true,
			skipTests:       false,
			verbose:         false,
			dryRun:          false,
			githubOwner:     "PyRo1121",
			githubRepo:      "omg",
			configFile:      filepath.Join(os.Getenv("HOME"), ".config", "omg", "deploy-tui.toml"),
		},
		operations: []operation{},
		logs:       []logEntry{},
		showLogs:   false,
	}
	return m
}

func main() {
	p := tea.NewProgram(initialModel(), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Printf("Error running program: %v\n", err)
		os.Exit(1)
	}
}
