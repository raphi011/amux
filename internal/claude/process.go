package claude

import (
	"os/exec"
	"strings"
)

// GetRunningClaudeWorkingDirs returns working directories and count of processes in each
func GetRunningClaudeWorkingDirs() (map[string]int, error) {
	// Get PIDs of running Claude processes
	cmd := exec.Command("bash", "-c", "ps aux | grep -E '\\bclaude\\b' | grep -v grep | grep -v amux | awk '{print $2}'")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	workingDirs := make(map[string]int)
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
					workingDirs[dir]++
					break
				}
			}
		}
	}

	return workingDirs, nil
}

// KillClaudeProcessesInDir kills all Claude processes running in the specified directory
func KillClaudeProcessesInDir(dir string) error {
	// Get PIDs of running Claude processes
	cmd := exec.Command("bash", "-c", "ps aux | grep -E '\\bclaude\\b' | grep -v grep | grep -v amux | awk '{print $2}'")
	output, err := cmd.Output()
	if err != nil {
		return err
	}

	pids := strings.Split(strings.TrimSpace(string(output)), "\n")
	killedAny := false

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
					processDir := strings.TrimPrefix(lines[i+1], "n")
					if processDir == dir {
						// Kill this process
						killCmd := exec.Command("kill", pid)
						_ = killCmd.Run() // Ignore errors
						killedAny = true
					}
					break
				}
			}
		}
	}

	if !killedAny {
		return nil // No error if no processes found
	}

	return nil
}
