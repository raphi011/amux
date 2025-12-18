package ui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/raphaelgruber/amux/internal/claude"
	"github.com/raphaelgruber/amux/internal/watcher"
)

// Update handles incoming messages and updates the model
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		return m.handleKeyPress(msg)

	case agentsLoadedMsg:
		m.agents = msg.agents
		m.err = msg.err
		m.loading = false
		m.lastUpdate = time.Now()

		// Reset cursor if it's out of bounds
		if m.cursor >= len(m.agents) && len(m.agents) > 0 {
			m.cursor = len(m.agents) - 1
		}
		if m.cursor < 0 {
			m.cursor = 0
		}

		// Load detail messages for selected agent
		if len(m.agents) > 0 {
			m.loadDetailMessages()
		}
		return m, nil

	case watcher.FileChangedMsg:
		// File changed, reload agents if not already loading
		var cmd tea.Cmd
		if !m.loading {
			m.loading = true
			m.lastUpdate = time.Now() // Update timestamp to show refresh happened
			cmd = loadAgentsCmd()
		}
		// Continue watching for next event
		if m.watcher != nil && m.ctx != nil {
			return m, tea.Batch(cmd, m.watcher.Start(m.ctx))
		}
		return m, cmd

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

		// Calculate viewport size based on terminal height for the list column
		// In split view: left column is 20% of width
		// Reserve space for: title bar (3 lines) in left column
		// Each agent takes 1 line now (simple single-line display)
		m.viewportSize = msg.Height - 4 // Reserve 4 lines for title bar + spacing
		if m.viewportSize < 1 {
			m.viewportSize = 1
		}

		// Calculate detail view height (reserve space for indicator at bottom)
		m.detailViewHeight = msg.Height - 5 // Reserve for title bar and scroll indicator
		if m.detailViewHeight < 1 {
			m.detailViewHeight = 1
		}

		// Rebuild lines with new width for proper wrapping
		if len(m.agents) > 0 && m.cursor < len(m.agents) {
			m.loadDetailMessages()
		}

		return m, nil
	}

	return m, nil
}

// handleKeyPress handles keyboard input
func (m Model) handleKeyPress(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Handle actual PgUp/PgDn keys by checking the key string
	keyStr := msg.String()

	// Debug - check what pgup/pgdn produce
	// These work in most terminals
	if strings.Contains(keyStr, "pgup") || strings.Contains(keyStr, "pageup") {
		m.detailViewportTop = max(0, m.detailViewportTop-m.detailViewHeight)
		return m, nil
	}
	if strings.Contains(keyStr, "pgdown") || strings.Contains(keyStr, "pagedown") {
		maxScroll := max(0, m.detailLineCount-m.detailViewHeight)
		m.detailViewportTop = min(maxScroll, m.detailViewportTop+m.detailViewHeight)
		return m, nil
	}

	// Always handle quit keys first
	switch keyStr {
	case "ctrl+c", "q", "esc":
		// Cleanup: stop watcher
		if m.watcher != nil && m.cancel != nil {
			m.cancel()
			m.watcher.Close()
		}
		return m, tea.Quit
	}

	// Handle refresh regardless of loading state or agent count
	if msg.String() == "r" {
		m.loading = true
		return m, loadAgentsCmd()
	}

	// Don't process navigation if loading
	if m.loading {
		return m, nil
	}

	// Don't process navigation if no agents
	if len(m.agents) == 0 {
		return m, nil
	}

	// Handle number keys for jumping to specific agent (1-9)
	if len(msg.String()) == 1 && msg.String()[0] >= '1' && msg.String()[0] <= '9' {
		num := int(msg.String()[0] - '0')
		if num > 0 && num <= len(m.agents) {
			m.cursor = num - 1
			// Adjust viewport to show cursor
			if m.cursor < m.agentViewportTop {
				m.agentViewportTop = m.cursor
			} else if m.cursor >= m.agentViewportTop+m.viewportSize {
				m.agentViewportTop = m.cursor - m.viewportSize + 1
			}
			m.loadDetailMessages()
			m.detailViewportTop = 0
		}
		return m, nil
	}

	// Handle navigation and other keys
	switch msg.String() {
	case "up", "k":
		// Scroll detail view up by one line
		if m.detailViewportTop > 0 {
			m.detailViewportTop--
		}

	case "down", "j":
		// Scroll detail view down by one line
		maxScroll := max(0, m.detailLineCount-m.detailViewHeight)
		if m.detailViewportTop < maxScroll {
			m.detailViewportTop++
		}

	case "g":
		// Jump to top of conversation
		m.detailViewportTop = 0

	case "G":
		// Jump to bottom of conversation
		m.detailViewportTop = max(0, m.detailLineCount-m.detailViewHeight)

	case "ctrl+b", "b":
		// Scroll up by viewport height (ctrl+b or b key)
		m.detailViewportTop = max(0, m.detailViewportTop-m.detailViewHeight)

	case "ctrl+f", "f", " ":
		// Scroll down by viewport height (ctrl+f, f, or space)
		maxScroll := max(0, m.detailLineCount-m.detailViewHeight)
		m.detailViewportTop = min(maxScroll, m.detailViewportTop+m.detailViewHeight)

	case "home":
		// Jump to top of conversation
		m.detailViewportTop = 0

	case "end":
		// Jump to bottom of conversation
		m.detailViewportTop = max(0, m.detailLineCount-m.detailViewHeight)

	case "x", "X":
		// Kill the selected Claude Code process
		if len(m.agents) > 0 && m.cursor < len(m.agents) {
			agent := m.agents[m.cursor]
			// Kill all Claude processes in this agent's project directory
			_ = claude.KillClaudeProcessesInDir(agent.ProjectPath)
			// Refresh the agent list after killing
			m.loading = true
			return m, loadAgentsCmd()
		}
		return m, nil
	}

	return m, nil
}

// max returns the larger of two integers
func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

// loadDetailMessages loads and formats messages for the currently selected agent
func (m *Model) loadDetailMessages() {
	if len(m.agents) == 0 || m.cursor >= len(m.agents) {
		m.detailLines = []string{"No agent selected"}
		m.detailLineCount = 1
		m.detailViewportTop = 0
		return
	}

	agent := m.agents[m.cursor]
	entries, err := claude.ParseJSONL(agent.JSONLPath)
	if err != nil {
		m.detailLines = []string{fmt.Sprintf("Error loading messages: %v", err)}
		m.detailLineCount = 1
		m.detailViewportTop = 0
		return
	}

	// Calculate content width for wrapping (70% of terminal, minus padding)
	contentWidth := m.width*70/100 - 4
	if contentWidth < 40 {
		contentWidth = 40 // Minimum width
	}

	// Build flat array of all lines
	var allLines []string

	// Format each message (in reverse order - newest first)
	for i := len(entries) - 1; i >= 0; i-- {
		entry := entries[i]

		// Skip messages without a role (system messages)
		role := entry.Message.Role
		if role == "" {
			continue
		}

		// Extract text content (handles both string and array formats)
		content := entry.GetContentText()

		// Skip messages with no text content
		if content == "" {
			continue
		}

		// Format header: "[timestamp] ROLE:"
		timeStr := messageTimeStyle.Render("[" + entry.Timestamp.Format("15:04:05") + "]")
		var roleStr string
		if role == "user" {
			roleStr = userRoleStyle.Render("USER")
		} else {
			roleStr = assistantRoleStyle.Render("ASSISTANT")
		}
		header := fmt.Sprintf("%s %s:", timeStr, roleStr)
		allLines = append(allLines, header)

		// Wrap and add content lines
		contentLines := strings.Split(content, "\n")
		for _, contentLine := range contentLines {
			if role == "user" {
				contentLine = userMessageStyle.Render(contentLine)
			} else {
				contentLine = assistantMessageStyle.Render(contentLine)
			}
			wrapped := wrapText(contentLine, contentWidth)
			allLines = append(allLines, wrapped...)
		}

		// Add visual separator between messages
		allLines = append(allLines, "")
		allLines = append(allLines, "─────────────────")
		allLines = append(allLines, "")
	}

	// Store lines
	oldLineCount := m.detailLineCount
	m.detailLines = allLines
	m.detailLineCount = len(allLines)

	// Implement smart scroll:
	// - If at top (viewing newest, position <= 10), stay at top
	// - If scrolled away, try to preserve relative position
	wasAtTop := m.detailViewportTop <= 10

	if wasAtTop {
		// Stay at top to see newest messages (tail -f behavior)
		m.detailViewportTop = 0
	} else if oldLineCount > 0 && m.detailLineCount > oldLineCount {
		// New lines added: keep same offset from old end
		// This preserves position when viewing history
		offset := oldLineCount - m.detailViewportTop
		m.detailViewportTop = max(0, m.detailLineCount - offset)
	}

	// Always clamp to valid range
	maxScroll := max(0, m.detailLineCount-m.detailViewHeight)
	m.detailViewportTop = min(m.detailViewportTop, maxScroll)
}
