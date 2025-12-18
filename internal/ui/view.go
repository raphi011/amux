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

	// No agents
	if len(m.agents) == 0 {
		if m.loading {
			s.WriteString("Loading...")
		} else {
			s.WriteString("No active sessions")
		}
		s.WriteString("\n")
		return s.String()
	}

	// Render all agents - simple list
	for i, ag := range m.agents {
		cursor := " " // no cursor
		if m.cursor == i {
			cursor = ">" // cursor!
		}

		// Line format: "> 1. project-name"
		line := fmt.Sprintf("%s %d. %s", cursor, i+1, ag.ProjectName)

		if m.cursor == i {
			s.WriteString(selectedRowStyle.Render(line))
		} else {
			s.WriteString(line)
		}
		s.WriteString("\n")

		// Second line with details
		details := fmt.Sprintf("     %s  󰀘 %s",
			ag.GitBranch,
			agent.FormatTokenCount(ag.TokensUsed))

		if ag.GitBranch == "" {
			details = fmt.Sprintf("     󰀘 %s", agent.FormatTokenCount(ag.TokensUsed))
		}

		s.WriteString(agentIDStyle.Render(details))
		s.WriteString("\n")
	}

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
