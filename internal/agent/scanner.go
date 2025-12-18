package agent

import (
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/raphaelgruber/claude-manager/internal/claude"
)

// ScanAgents scans for all Claude Code agents and returns their information
func ScanAgents() ([]Agent, error) {
	projectsDir, err := claude.GetProjectsDir()
	if err != nil {
		return nil, err
	}

	// Map to store agents by their ID to avoid duplicates
	agentMap := make(map[string]*Agent)

	// Scan all project directories
	projects, err := os.ReadDir(projectsDir)
	if err != nil {
		return nil, err
	}

	for _, project := range projects {
		if !project.IsDir() {
			continue
		}

		projectPath := filepath.Join(projectsDir, project.Name())
		files, err := os.ReadDir(projectPath)
		if err != nil {
			continue
		}

		// Look for agent JSONL files
		for _, file := range files {
			if !strings.HasPrefix(file.Name(), "agent-") || !strings.HasSuffix(file.Name(), ".jsonl") {
				continue
			}

			jsonlPath := filepath.Join(projectPath, file.Name())
			agent, err := parseAgentFromJSONL(jsonlPath)
			if err != nil || agent == nil {
				continue
			}

			// Store or update agent info
			if existing, ok := agentMap[agent.ID]; !ok || agent.LastActive.After(existing.LastActive) {
				agentMap[agent.ID] = agent
			}
		}
	}

	// Get working directories of running Claude processes
	runningDirs, err := claude.GetRunningClaudeWorkingDirs()
	if err != nil || len(runningDirs) == 0 {
		return []Agent{}, nil
	}

	// Convert map to slice - only include agents from running directories
	agents := make([]Agent, 0)
	for _, agent := range agentMap {
		// Load todo information
		loadTodoInfo(agent)

		// Only include if this agent's project matches a running directory
		if runningDirs[agent.ProjectPath] {
			agent.IsActive = true
			agents = append(agents, *agent)
		}
	}

	// Sort agents: active first, then by last active time
	sort.Slice(agents, func(i, j int) bool {
		if agents[i].IsActive != agents[j].IsActive {
			return agents[i].IsActive
		}
		return agents[i].LastActive.After(agents[j].LastActive)
	})

	return agents, nil
}

// parseAgentFromJSONL extracts agent information from a JSONL file
func parseAgentFromJSONL(filePath string) (*Agent, error) {
	// Parse all entries to get token totals
	entries, err := claude.ParseJSONL(filePath)
	if err != nil || len(entries) == 0 {
		return nil, err
	}

	lastEntry := &entries[len(entries)-1]

	// Calculate total tokens used
	totalInput, totalOutput := claude.CalculateTotalTokens(entries)

	// Extract agent ID from filename
	filename := filepath.Base(filePath)
	agentID := strings.TrimPrefix(filename, "agent-")
	agentID = strings.TrimSuffix(agentID, ".jsonl")

	agent := &Agent{
		ID:          agentID,
		Slug:        lastEntry.Slug,
		SessionID:   lastEntry.SessionID,
		ProjectPath: lastEntry.CWD,
		LastActive:  lastEntry.Timestamp,
		CurrentTask: "Loading...",
		TaskStatus:  "unknown",
		TokensUsed:  totalInput + totalOutput,
		TokensInput: totalInput,
	}

	return agent, nil
}

// loadTodoInfo loads todo information for an agent
func loadTodoInfo(agent *Agent) {
	// Todo files are named by session ID, not agent ID
	todoFile, err := claude.FindTodoFile(agent.SessionID)
	if err != nil || todoFile == "" {
		agent.CurrentTask = "No tasks"
		agent.TaskStatus = "unknown"
		return
	}

	todos, err := claude.ParseTodoFile(todoFile)
	if err != nil {
		agent.CurrentTask = "Error loading tasks"
		agent.TaskStatus = "unknown"
		return
	}

	task, status := claude.GetCurrentTask(todos)
	agent.CurrentTask = task
	agent.TaskStatus = status
}
