package ui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/raphaelgruber/claude-manager/internal/agent"
)

// View renders the UI
func (m Model) View() string {
	if m.width == 0 {
		return "Loading..."
	}

	// Calculate column widths (20% for list, 80% for detail)
	listWidth := m.width * 20 / 100
	detailWidth := m.width - listWidth - 1 // -1 for separator

	// Render left column (agent list)
	leftColumn := m.renderAgentList(listWidth)

	// Render right column (detail view)
	rightColumn := m.renderDetailView(detailWidth)

	// Combine columns side by side
	return lipgloss.JoinHorizontal(
		lipgloss.Top,
		leftColumn,
		lipgloss.NewStyle().Foreground(colorGray).Render("│"),
		rightColumn,
	)
}

// renderAgentList renders the left column with agent list
func (m Model) renderAgentList(width int) string {
	var s strings.Builder

	// Title
	title := titleStyle.Render("Running Sessions")
	s.WriteString(title)
	s.WriteString("\n")
	s.WriteString(separatorStyle.Render(strings.Repeat("─", width-2)))
	s.WriteString("\n")

	// Loading state (but still show agents if we have them)
	if m.loading && len(m.agents) == 0 {
		s.WriteString(loadingStyle.Render("Loading agents..."))
		s.WriteString("\n")
		return s.String()
	}

	// Error state
	if m.err != nil {
		s.WriteString(errorStyle.Render(fmt.Sprintf("Error: %v", m.err)))
		s.WriteString("\n")
		return s.String()
	}

	// No agents
	if len(m.agents) == 0 {
		s.WriteString(loadingStyle.Render("No running Claude Code processes detected"))
		s.WriteString("\n")
		s.WriteString(agentIDStyle.Render("  (showing all sessions when Claude is running)"))
		s.WriteString("\n")
		s.WriteString("\n")
		s.WriteString(helpBarStyle.Render("[r] Refresh  [q] Quit"))
		s.WriteString("\n")
		return s.String()
	}

	// Calculate viewport bounds
	viewportEnd := m.viewportTop + m.viewportSize
	if viewportEnd > len(m.agents) {
		viewportEnd = len(m.agents)
	}

	// Show scroll indicator if there are more items above
	if m.viewportTop > 0 {
		s.WriteString(agentIDStyle.Render(fmt.Sprintf("  ↑ %d more above...\n\n", m.viewportTop)))
	}

	// Render visible agents only
	for i := m.viewportTop; i < viewportEnd; i++ {
		s.WriteString(m.renderAgent(m.agents[i], i == m.cursor))
		if i < viewportEnd-1 {
			s.WriteString("\n")
		}
	}

	// Show scroll indicator if there are more items below
	if viewportEnd < len(m.agents) {
		remaining := len(m.agents) - viewportEnd
		s.WriteString("\n\n")
		s.WriteString(agentIDStyle.Render(fmt.Sprintf("  ↓ %d more below...", remaining)))
	}

	// Help bar
	s.WriteString("\n")
	s.WriteString(separatorStyle.Render(strings.Repeat("─", 80)))
	s.WriteString("\n")

	helpText := "[↑↓/jk] Navigate  [g/G] Top/Bottom  [r] Refresh  [a] Auto-refresh: "
	if m.autoRefresh {
		helpText += "ON"
	} else {
		helpText += "OFF"
	}
	helpText += "  [q] Quit"
	if m.loading {
		helpText += "  " + loadingStyle.Render("⟳ Refreshing...")
	}
	s.WriteString(helpBarStyle.Render(helpText))
	s.WriteString("\n")

	return s.String()
}

// renderAgent renders a single agent row
func (m Model) renderAgent(ag agent.Agent, selected bool) string {
	var s strings.Builder

	// Build the content
	var content strings.Builder

	// Add cursor indicator for selected row
	if selected {
		content.WriteString("> ")
	} else {
		content.WriteString("  ")
	}

	// First line: indicator and folder name
	if ag.IsActive {
		content.WriteString(activeIndicator.Render())
	} else {
		content.WriteString(inactiveIndicator.Render())
	}
	content.WriteString(" ")

	// Show project folder name
	content.WriteString(agentNameStyle.Render(ag.ProjectName))

	if !ag.IsActive {
		content.WriteString(" ")
		content.WriteString(agentIDStyle.Render("[INACTIVE]"))
	}
	content.WriteString("\n")

	// Second line: git branch
	content.WriteString("    ")
	if ag.GitBranch != "" {
		content.WriteString(agentIDStyle.Render("Branch: "))
		content.WriteString(projectStyle.Render(ag.GitBranch))
	} else {
		content.WriteString(agentIDStyle.Render("Branch: "))
		content.WriteString(agentIDStyle.Render("(no branch)"))
	}
	content.WriteString("\n")

	// Third line: current task
	content.WriteString("    ")
	content.WriteString(agentIDStyle.Render("Task: "))
	content.WriteString(GetTaskStatusStyle(ag.TaskStatus).Render())
	content.WriteString(" ")
	content.WriteString(taskContentStyle.Render(truncateString(ag.CurrentTask, 60)))
	content.WriteString("\n")

	// Fourth line: last active
	content.WriteString("    ")
	content.WriteString(agentIDStyle.Render("Last active: "))
	content.WriteString(timeAgoStyle.Render(ag.TimeSinceActive()))
	content.WriteString("\n")

	// Fifth line: token usage
	content.WriteString("    ")
	content.WriteString(agentIDStyle.Render("Tokens: "))
	content.WriteString(projectStyle.Render(agent.FormatTokenCount(ag.TokensUsed)))
	content.WriteString(agentIDStyle.Render(" (input: "))
	content.WriteString(projectStyle.Render(agent.FormatTokenCount(ag.TokensInput)))
	content.WriteString(agentIDStyle.Render(")"))

	// Apply selection style if selected
	if selected {
		s.WriteString(selectedRowStyle.Render(content.String()))
	} else {
		s.WriteString(content.String())
	}

	s.WriteString("\n")

	return s.String()
}

// shortenPath shortens a path by replacing the home directory with ~
func shortenPath(path string) string {
	if path == "" {
		return "Unknown"
	}
	// Simple implementation - just show the last 2 parts
	parts := strings.Split(path, "/")
	if len(parts) > 2 {
		return "~/" + strings.Join(parts[len(parts)-2:], "/")
	}
	return path
}

// truncateString truncates a string to a maximum length
func truncateString(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
