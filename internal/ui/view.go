package ui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/raphaelgruber/amux/internal/agent"
)

// View renders the UI
func (m Model) View() string {
	if m.width == 0 {
		return "Loading..."
	}

	var s strings.Builder

	// Render title bar spanning full width
	titleText := agentNameStyle.Render("amux")
	titleText += agentIDStyle.Render("  sessions")
	titleText += agentIDStyle.Render(" • ")
	titleText += agentIDStyle.Render("kill")
	s.WriteString(titleText)
	s.WriteString("\n\n")

	// Calculate column widths (20% for list, 80% for detail)
	listWidth := m.width * 20 / 100
	detailWidth := m.width - listWidth - 1 // -1 for separator

	// Render left column (agent list - without title)
	leftColumn := m.renderAgentList(listWidth)

	// Render right column (detail view)
	rightColumn := m.renderDetailView(detailWidth)

	// Count lines in each column
	leftLines := strings.Count(leftColumn, "\n")
	rightLines := strings.Count(rightColumn, "\n")

	// Pad the shorter column to match heights
	if leftLines < rightLines {
		leftColumn += strings.Repeat("\n", rightLines-leftLines)
	} else if rightLines < leftLines {
		rightColumn += strings.Repeat("\n", leftLines-rightLines)
	}

	// Combine columns side by side
	s.WriteString(lipgloss.JoinHorizontal(
		lipgloss.Top,
		leftColumn,
		lipgloss.NewStyle().Foreground(colorGray).Render("│"),
		rightColumn,
	))

	return s.String()
}

// renderAgentList renders the left column with agent list
func (m Model) renderAgentList(width int) string {
	var s strings.Builder

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

	// Render all agents (no viewport scrolling - simpler, always show all)
	for i := 0; i < len(m.agents); i++ {
		s.WriteString(m.renderAgent(m.agents[i], i == m.cursor, i))
	}

	return s.String()
}

// renderAgent renders a single agent row (single line, circumflex-style)
func (m Model) renderAgent(ag agent.Agent, selected bool, index int) string {
	var line strings.Builder

	// Number with padding
	numStr := fmt.Sprintf("%2d. ", index+1)
	if selected {
		line.WriteString(agentNameStyle.Render(numStr))
	} else {
		line.WriteString(agentIDStyle.Render(numStr))
	}

	// Project name
	projectName := ag.ProjectName
	if len(projectName) > 30 {
		projectName = projectName[:27] + "..."
	}
	if selected {
		line.WriteString(selectedRowStyle.Render(projectName))
	} else {
		line.WriteString(projectName)
	}

	// Branch if available
	if ag.GitBranch != "" {
		line.WriteString(agentIDStyle.Render(" ("))
		line.WriteString(projectStyle.Render(ag.GitBranch))
		line.WriteString(agentIDStyle.Render(")"))
	}

	line.WriteString("\n")
	return line.String()
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
