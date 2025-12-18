package agent

import "time"

// Agent represents a Claude Code agent instance
type Agent struct {
	ID          string    // Agent ID (e.g., "a1a0698")
	Slug        string    // Human-readable slug (e.g., "typed-doodling-meerkat")
	SessionID   string    // Session UUID
	ProjectPath string    // Working directory
	ProjectName string    // Just the folder name
	GitBranch   string    // Current git branch
	LastActive  time.Time // Last message timestamp
	CurrentTask string    // Current task from todo file
	TaskStatus  string    // "pending", "in_progress", "completed", or "unknown"
	IsActive    bool      // Active within last 5 minutes
	TokensUsed  int       // Total tokens used (input + output)
	TokensInput int       // Input tokens
	JSONLPath   string    // Path to JSONL file for this agent
}

// IsRecentlyActive checks if the agent was active in the last 15 minutes
func (a *Agent) IsRecentlyActive() bool {
	return time.Since(a.LastActive) < 15*time.Minute
}

// TimeSinceActive returns a human-readable string of time since last activity
func (a *Agent) TimeSinceActive() string {
	duration := time.Since(a.LastActive)

	switch {
	case duration < time.Minute:
		return "just now"
	case duration < 2*time.Minute:
		return "1m ago"
	case duration < time.Hour:
		return formatMinutes(duration)
	case duration < 2*time.Hour:
		return "1h ago"
	case duration < 24*time.Hour:
		return formatHours(duration)
	default:
		return formatDays(duration)
	}
}

func formatMinutes(d time.Duration) string {
	mins := int(d.Minutes())
	return formatDuration(mins, "m")
}

func formatHours(d time.Duration) string {
	hours := int(d.Hours())
	return formatDuration(hours, "h")
}

func formatDays(d time.Duration) string {
	days := int(d.Hours() / 24)
	return formatDuration(days, "d")
}

func formatDuration(value int, unit string) string {
	return formatValue(value) + unit + " ago"
}

func formatValue(v int) string {
	if v < 10 {
		return string(rune('0' + v))
	}
	return intToString(v)
}

func intToString(n int) string {
	if n == 0 {
		return "0"
	}

	var result []byte
	for n > 0 {
		result = append([]byte{byte('0' + n%10)}, result...)
		n /= 10
	}
	return string(result)
}

// FormatTokenCount returns a human-readable token count with commas
func FormatTokenCount(n int) string {
	if n < 1000 {
		return intToString(n)
	}
	if n < 1000000 {
		return intToString(n/1000) + "," + padLeft(intToString(n%1000), 3)
	}
	return intToString(n/1000000) + "," + padLeft(intToString((n%1000000)/1000), 3) + "," + padLeft(intToString(n%1000), 3)
}

func padLeft(s string, length int) string {
	for len(s) < length {
		s = "0" + s
	}
	return s
}
