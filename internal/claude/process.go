package claude

import (
	"os/exec"
	"strings"
)

// GetRunningClaudeWorkingDirs returns a set of working directories for running Claude processes
func GetRunningClaudeWorkingDirs() (map[string]bool, error) {
	// Get PIDs of running Claude processes
	cmd := exec.Command("bash", "-c", "ps aux | grep -E '\\bclaude\\b' | grep -v grep | grep -v claude-manager | awk '{print $2}'")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	workingDirs := make(map[string]bool)
	pids := strings.Split(strings.TrimSpace(string(output)), "\n")

	for _, pid := range pids {
		if pid == "" {
			continue
		}

		// Get working directory for this PID using lsof
		lsofCmd := exec.Command("lsof", "-p", pid, "-Fn")
		lsofOutput, err := lsofCmd.Output()
		if err != nil {
			continue
		}

		// Parse lsof output to find cwd
		lines := strings.Split(string(lsofOutput), "\n")
		for i, line := range lines {
			if strings.HasPrefix(line, "fcwd") {
				// Next line should be the directory path
				if i+1 < len(lines) && strings.HasPrefix(lines[i+1], "n") {
					dir := strings.TrimPrefix(lines[i+1], "n")
					workingDirs[dir] = true
					break
				}
			}
		}
	}

	return workingDirs, nil
}
