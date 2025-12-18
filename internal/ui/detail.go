package ui

import (
	"fmt"
	"strings"
)

// renderDetailView renders the right column with message detail
func (m Model) renderDetailView(width int) string {
	var s strings.Builder

	// Title showing selected agent
	if len(m.agents) > 0 && m.cursor < len(m.agents) {
		agent := m.agents[m.cursor]
		displayName := agent.Slug
		if displayName == "" {
			displayName = agent.ID
		}
		title := titleStyle.Render(fmt.Sprintf("Messages: %s", displayName))
		s.WriteString(title)
	} else {
		s.WriteString(titleStyle.Render("Messages"))
	}
	s.WriteString("\n")
	s.WriteString(separatorStyle.Render(strings.Repeat("─", width-2)))
	s.WriteString("\n")

	// Calculate how many lines we can show
	availableHeight := m.height - 4 // Reserve space for title, separator, help

	// Render messages
	if len(m.detailMessages) == 0 {
		s.WriteString(agentIDStyle.Render("No messages"))
		s.WriteString("\n")
	} else {
		// Calculate visible message range
		start := m.detailScroll
		linesShown := 0

		for i := start; i < len(m.detailMessages) && linesShown < availableHeight; i++ {
			msg := m.detailMessages[i]
			lines := strings.Split(msg, "\n")

			for _, line := range lines {
				if linesShown >= availableHeight {
					break
				}
				// Wrap long lines to fit width
				wrapped := wrapText(line, width-4)
				for _, wl := range wrapped {
					if linesShown >= availableHeight {
						break
					}
					s.WriteString("  ")
					s.WriteString(wl)
					s.WriteString("\n")
					linesShown++
				}
			}
		}
	}

	// Add scroll indicator if needed
	if len(m.detailMessages) > 0 {
		s.WriteString("\n")
		s.WriteString(agentIDStyle.Render(fmt.Sprintf("Message %d/%d", m.detailScroll+1, len(m.detailMessages))))
	}

	return s.String()
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
