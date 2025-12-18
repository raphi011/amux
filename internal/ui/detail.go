package ui

import (
	"fmt"
	"strings"
)

// renderDetailView renders the right column with message detail
func (m Model) renderDetailView(width int) string {
	var s strings.Builder

	if len(m.detailLines) == 0 {
		s.WriteString(agentIDStyle.Render("No messages"))
		s.WriteString("\n")
		return s.String()
	}

	// Calculate visible range
	start := m.detailViewportTop
	end := min(start+m.detailViewHeight, m.detailLineCount)

	// Render visible lines
	for i := start; i < end; i++ {
		s.WriteString("  ") // Left padding
		s.WriteString(m.detailLines[i])
		s.WriteString("\n")
	}

	// Scroll position indicator
	indicator := fmt.Sprintf("Lines %d-%d/%d", start+1, end, m.detailLineCount)
	s.WriteString("\n")
	s.WriteString(agentIDStyle.Render(indicator))

	return s.String()
}

// min returns the smaller of two integers
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// wrapText wraps text to fit within the specified width
func wrapText(text string, width int) []string {
	if len(text) <= width {
		return []string{text}
	}

	var lines []string
	for len(text) > width {
		// Find last space before width
		breakPoint := width
		for breakPoint > 0 && text[breakPoint] != ' ' {
			breakPoint--
		}
		if breakPoint == 0 {
			// No space found, hard break
			breakPoint = width
		}

		lines = append(lines, text[:breakPoint])
		text = strings.TrimLeft(text[breakPoint:], " ")
	}

	if len(text) > 0 {
		lines = append(lines, text)
	}

	return lines
}
