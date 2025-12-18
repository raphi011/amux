package ui

import "github.com/charmbracelet/lipgloss"

var (
	// Color scheme
	colorGreen   = lipgloss.Color("#00FF00")
	colorGray    = lipgloss.Color("#888888")
	colorCyan    = lipgloss.Color("#00FFFF")
	colorYellow  = lipgloss.Color("#FFFF00")
	colorBlue    = lipgloss.Color("#0088FF")
	colorRed     = lipgloss.Color("#FF0000")
	colorWhite   = lipgloss.Color("#FFFFFF")
	colorBgLight = lipgloss.Color("#333333")

	// Title style
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorCyan).
			MarginBottom(1)

	// Separator style
	separatorStyle = lipgloss.NewStyle().
			Foreground(colorGray)

	// Active indicator (green circle)
	activeIndicator = lipgloss.NewStyle().
			Foreground(colorGreen).
			SetString("●")

	// Inactive indicator (gray circle)
	inactiveIndicator = lipgloss.NewStyle().
				Foreground(colorGray).
				SetString("○")

	// Agent name style
	agentNameStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorWhite)

	// Agent ID style
	agentIDStyle = lipgloss.NewStyle().
			Foreground(colorGray)

	// Project path style
	projectStyle = lipgloss.NewStyle().
			Foreground(colorCyan)

	// Task label styles
	taskLabelPending = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorYellow).
				SetString("[PENDING]")

	taskLabelInProgress = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorGreen).
				SetString("[IN PROGRESS]")

	taskLabelCompleted = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorBlue).
				SetString("[COMPLETED]")

	taskLabelUnknown = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorGray).
				SetString("[UNKNOWN]")

	// Task content style
	taskContentStyle = lipgloss.NewStyle().
				Foreground(colorWhite)

	// Time ago style
	timeAgoStyle = lipgloss.NewStyle().
			Foreground(colorGray).
			Italic(true)

	// Selected row style
	selectedRowStyle = lipgloss.NewStyle().
				Background(colorBgLight)

	// Help bar style
	helpBarStyle = lipgloss.NewStyle().
			Foreground(colorGray).
			MarginTop(1)

	// Error style
	errorStyle = lipgloss.NewStyle().
			Foreground(colorRed).
			Bold(true)

	// Loading style
	loadingStyle = lipgloss.NewStyle().
			Foreground(colorYellow)
)

// GetTaskStatusStyle returns the appropriate style for a task status
func GetTaskStatusStyle(status string) lipgloss.Style {
	switch status {
	case "pending":
		return taskLabelPending
	case "in_progress":
		return taskLabelInProgress
	case "completed":
		return taskLabelCompleted
	default:
		return taskLabelUnknown
	}
}
