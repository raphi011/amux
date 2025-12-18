package claude

import (
	"os/exec"
	"strconv"
	"strings"
)

// HasRunningClaudeSessions checks if there are any running Claude Code processes
func HasRunningClaudeSessions() bool {
	cmd := exec.Command("ps", "aux")
	output, err := cmd.Output()
	if err != nil {
		return false
	}

	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		// Look for claude processes (not claude-manager)
		if strings.Contains(line, "claude") &&
			!strings.Contains(line, "grep") &&
			!strings.Contains(line, "claude-manager") {
			return true
		}
	}
	return false
}

// GetRunningClaudeProcessCount returns the number of running Claude processes
func GetRunningClaudeProcessCount() int {
	cmd := exec.Command("bash", "-c", "ps aux | grep claude | grep -v grep | grep -v claude-manager | wc -l")
	output, err := cmd.Output()
	if err != nil {
		return 0
	}

	count, err := strconv.Atoi(strings.TrimSpace(string(output)))
	if err != nil {
		return 0
	}
	return count
}
