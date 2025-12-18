package ui

import (
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/raphaelgruber/claude-manager/internal/claude"
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

	case tickMsg:
		// Auto-refresh: reload agents (only if enabled and not currently loading)
		if m.autoRefresh && !m.loading {
			m.loading = true
			return m, tea.Batch(loadAgentsCmd(), tickCmd())
		}
		// Schedule next tick
		return m, tickCmd()

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

		// Calculate viewport size based on terminal height
		// Reserve space for: title (3 lines) + separator (1) + help bar (2) = 6 lines
		// Each agent takes ~6 lines (name + project + task + last active + tokens + blank)
		m.viewportSize = (msg.Height - 6) / 6
		if m.viewportSize < 1 {
			m.viewportSize = 1
		}
		return m, nil
	}

	return m, nil
}

// handleKeyPress handles keyboard input
func (m Model) handleKeyPress(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Always handle quit keys first
	switch msg.String() {
	case "ctrl+c", "q", "esc":
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

	// Handle navigation and other keys
	switch msg.String() {
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
			// Scroll up if cursor moves above viewport
			if m.cursor < m.viewportTop {
				m.viewportTop = m.cursor
			}
			m.loadDetailMessages()
			m.detailScroll = 0
		}

	case "down", "j":
		if m.cursor < len(m.agents)-1 {
			m.cursor++
			// Scroll down if cursor moves below viewport
			if m.cursor >= m.viewportTop+m.viewportSize {
				m.viewportTop = m.cursor - m.viewportSize + 1
			}
			m.loadDetailMessages()
			m.detailScroll = 0
		}

	case "g":
		// Go to top
		m.cursor = 0
		m.viewportTop = 0

	case "G":
		// Go to bottom
		if len(m.agents) > 0 {
			m.cursor = len(m.agents) - 1
			// Adjust viewport to show bottom
			m.viewportTop = max(0, m.cursor-m.viewportSize+1)
		}

	case "a":
		// Toggle auto-refresh
		m.autoRefresh = !m.autoRefresh

	case "left", "h":
		// Scroll detail up
		if m.detailScroll > 0 {
			m.detailScroll--
		}

	case "right", "l":
		// Scroll detail down
		if m.detailScroll < len(m.detailMessages)-1 {
			m.detailScroll++
		}

	case "x", "delete":
		// Kill agent (placeholder for future implementation)
		// TODO: Implement agent killing functionality
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
		m.detailMessages = []string{"No agent selected"}
		m.detailScroll = 0
		return
	}

	agent := m.agents[m.cursor]
	entries, err := claude.ParseJSONL(agent.JSONLPath)
	if err != nil {
		m.detailMessages = []string{fmt.Sprintf("Error loading messages: %v", err)}
		m.detailScroll = 0
		return
	}

	// Format each message (in reverse order - newest first)
	messages := make([]string, 0, len(entries))
	for i := len(entries) - 1; i >= 0; i-- {
		entry := entries[i]

		// Skip messages without a role (system messages)
		role := entry.Message.Role
		if role == "" {
			continue
		}

		// Extract text content (from both "text" and other types)
		var content string
		for _, c := range entry.Message.Content {
			if c.Type == "text" && c.Text != "" {
				content += c.Text
			}
		}

		// Skip messages with no text content
		if content == "" {
			continue
		}

		// Format timestamp with color
		timeStr := messageTimeStyle.Render("[" + entry.Timestamp.Format("15:04:05") + "]")

		// Format role with color
		var roleStr string
		var contentStyled string
		if role == "user" {
			roleStr = userRoleStyle.Render("USER")
			contentStyled = userMessageStyle.Render(content)
		} else {
			roleStr = assistantRoleStyle.Render("ASSISTANT")
			contentStyled = assistantMessageStyle.Render(content)
		}

		// Create formatted message
		msg := fmt.Sprintf("%s %s:\n%s\n", timeStr, roleStr, contentStyled)
		messages = append(messages, msg)
	}

	m.detailMessages = messages

	// Start at the first message (which is now the newest)
	m.detailScroll = 0
}
