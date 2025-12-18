package ui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
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
		}

	case "down", "j":
		if m.cursor < len(m.agents)-1 {
			m.cursor++
		}

	case "g":
		// Go to top
		m.cursor = 0

	case "G":
		// Go to bottom
		if len(m.agents) > 0 {
			m.cursor = len(m.agents) - 1
		}

	case "a":
		// Toggle auto-refresh
		m.autoRefresh = !m.autoRefresh

	case "x", "delete":
		// Kill agent (placeholder for future implementation)
		// TODO: Implement agent killing functionality
		return m, nil
	}

	return m, nil
}
