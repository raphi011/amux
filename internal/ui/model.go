package ui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/raphaelgruber/claude-manager/internal/agent"
)

// Model represents the Bubbletea application model
type Model struct {
	agents       []agent.Agent
	cursor       int
	viewportTop  int // First visible item index
	viewportSize int // Number of items that fit on screen
	lastUpdate   time.Time
	err          error
	loading      bool
	autoRefresh  bool
}

// tickMsg is sent on every auto-refresh tick
type tickMsg time.Time

// agentsLoadedMsg is sent when agents are loaded
type agentsLoadedMsg struct {
	agents []agent.Agent
	err    error
}

// NewModel creates a new Model instance
func NewModel() Model {
	return Model{
		agents:       []agent.Agent{},
		cursor:       0,
		viewportTop:  0,
		viewportSize: 10, // Default, will be updated based on terminal size
		lastUpdate:   time.Now(),
		loading:      true,
		autoRefresh:  false, // Start with auto-refresh disabled
	}
}

// Init initializes the model
func (m Model) Init() tea.Cmd {
	return tea.Batch(
		loadAgentsCmd(),
		tickCmd(),
	)
}

// loadAgentsCmd loads agents asynchronously
func loadAgentsCmd() tea.Cmd {
	return func() tea.Msg {
		agents, err := agent.ScanAgents()
		return agentsLoadedMsg{agents: agents, err: err}
	}
}

// tickCmd returns a command that sends a tick message every 10 seconds
func tickCmd() tea.Cmd {
	return tea.Tick(10*time.Second, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}
