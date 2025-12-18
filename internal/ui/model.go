package ui

import (
	"context"
	"os"
	"path/filepath"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/raphaelgruber/amux/internal/agent"
	"github.com/raphaelgruber/amux/internal/watcher"
)

// Model represents the Bubbletea application model
type Model struct {
	agents            []agent.Agent
	cursor            int
	agentViewportTop  int      // First visible agent index (renamed from viewportTop)
	viewportSize      int      // Number of agents that fit on screen
	lastUpdate        time.Time
	err               error
	loading           bool
	width             int      // Terminal width
	height            int      // Terminal height
	detailLines       []string // Flattened array of all conversation lines
	detailLineCount   int      // Total lines in conversation
	detailViewportTop int      // First visible line index
	detailViewHeight  int      // Number of visible lines in viewport
	watcher           *watcher.FileWatcher
	ctx               context.Context
	cancel            context.CancelFunc
}

// agentsLoadedMsg is sent when agents are loaded
type agentsLoadedMsg struct {
	agents []agent.Agent
	err    error
}

// NewModel creates a new Model instance
func NewModel() Model {
	return Model{
		agents:            []agent.Agent{},
		cursor:            0,
		agentViewportTop:  0,
		viewportSize:      10, // Default, will be updated based on terminal size
		lastUpdate:        time.Now(),
		loading:           true,
		detailViewportTop: 0,
	}
}

// Init initializes the model
func (m Model) Init() tea.Cmd {
	// Get Claude directories
	home := os.Getenv("HOME")
	if home == "" {
		// Fallback to manual refresh only
		return loadAgentsCmd()
	}

	claudeDir := filepath.Join(home, ".claude")
	projectsDir := filepath.Join(claudeDir, "projects")
	todosDir := filepath.Join(claudeDir, "todos")

	// Initialize file watcher
	w, err := watcher.NewWatcher([]string{projectsDir, todosDir})
	if err != nil {
		// Fall back to manual refresh only if watcher fails
		return loadAgentsCmd()
	}

	m.watcher = w
	m.ctx, m.cancel = context.WithCancel(context.Background())

	return tea.Batch(
		loadAgentsCmd(),
		w.Start(m.ctx),
	)
}

// loadAgentsCmd loads agents asynchronously
func loadAgentsCmd() tea.Cmd {
	return func() tea.Msg {
		agents, err := agent.ScanAgents()
		return agentsLoadedMsg{agents: agents, err: err}
	}
}
