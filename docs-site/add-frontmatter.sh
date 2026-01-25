#!/bin/bash
# Add frontmatter to docs that don't have it

add_frontmatter() {
  local file="$1"
  local title="$2"
  local position="$3"
  local desc="$4"
  
  # Check if file already has frontmatter
  if head -1 "$file" | grep -q "^---"; then
    echo "Skipping $file (already has frontmatter)"
    return
  fi
  
  # Create temp file with frontmatter
  {
    echo "---"
    echo "title: $title"
    echo "sidebar_position: $position"
    echo "description: $desc"
    echo "---"
    echo ""
    cat "$file"
  } > "${file}.tmp"
  mv "${file}.tmp" "$file"
  echo "Added frontmatter to $file"
}

cd /home/pyro1121/Documents/code/filemanager/omg/docs-site/docs

add_frontmatter "index.md" "Introduction" "1" "The complete guide to the fastest unified package manager"
add_frontmatter "quickstart.md" "Quick Start" "2" "Get started with OMG in 5 minutes"
add_frontmatter "cli.md" "CLI Reference" "3" "Complete command reference for all OMG commands"
add_frontmatter "configuration.md" "Configuration" "4" "Configuration files, paths, and policy settings"
add_frontmatter "packages.md" "Package Management" "10" "Search, install, update, and remove packages"
add_frontmatter "runtimes.md" "Runtime Management" "11" "Managing Node.js, Python, Go, Rust, Ruby, Java, and Bun"
add_frontmatter "shell-integration.md" "Shell Integration" "12" "Hooks, completions, and PATH management"
add_frontmatter "task-runner.md" "Task Runner" "13" "Unified task execution across ecosystems"
add_frontmatter "security.md" "Security & Compliance" "20" "Vulnerability scanning, SBOM, PGP verification, and audit logging"
add_frontmatter "team.md" "Team Collaboration" "21" "Environment lockfiles, drift detection, and team sync"
add_frontmatter "containers.md" "Container Support" "22" "Docker and Podman integration"
add_frontmatter "tui.md" "TUI Dashboard" "23" "Interactive terminal dashboard for system monitoring"
add_frontmatter "history.md" "History & Rollback" "24" "Transaction history and system rollback"
add_frontmatter "architecture.md" "Architecture" "30" "System architecture and component overview"
add_frontmatter "daemon.md" "Daemon Internals" "31" "Background service lifecycle, IPC, and state management"
add_frontmatter "cache.md" "Caching & Indexing" "32" "In-memory and persistent caching strategies"
add_frontmatter "ipc.md" "IPC Protocol" "33" "Binary protocol for CLI-daemon communication"
add_frontmatter "package-search.md" "Package Search" "34" "Search indexing and ranking algorithms"
add_frontmatter "cli-internals.md" "CLI Internals" "35" "CLI implementation details and optimization"
add_frontmatter "workflows.md" "Workflows" "40" "Common workflows and recipes"
add_frontmatter "troubleshooting.md" "Troubleshooting" "50" "Common issues and solutions"
add_frontmatter "faq.md" "FAQ" "51" "Frequently asked questions"
add_frontmatter "changelog.md" "Changelog" "99" "Version history and release notes"

echo "Done!"
