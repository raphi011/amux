package agent

import (
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/raphaelgruber/amux/internal/claude"
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

	// Get working directories of running Claude processes with counts
	runningDirs, err := claude.GetRunningClaudeWorkingDirs()
	if err != nil || len(runningDirs) == 0 {
		return []Agent{}, nil
	}

	// Group agents by project directory
	agentsByProject := make(map[string][]*Agent)
	for _, agent := range agentMap {
		// Only consider agents from running directories
		if _, ok := runningDirs[agent.ProjectPath]; ok {
			agentsByProject[agent.ProjectPath] = append(agentsByProject[agent.ProjectPath], agent)
		}
	}

	// For each project, take the most recent agent
	agents := make([]Agent, 0)
	for dir := range runningDirs {
		projectAgents := agentsByProject[dir]

		if len(projectAgents) == 0 {
			continue
		}

		// Sort by last active time (newest first)
		sort.Slice(projectAgents, func(i, j int) bool {
			return projectAgents[i].LastActive.After(projectAgents[j].LastActive)
		})

		// Take only the most recent agent
		agent := projectAgents[0]
		loadTodoInfo(agent)
		agent.IsActive = true
		agents = append(agents, *agent)
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

	// Extract project name (last directory name)
	projectName := filepath.Base(lastEntry.CWD)
	if projectName == "" || projectName == "." {
		projectName = lastEntry.CWD
	}

	// Try to find the session JSONL file (contains full conversation)
	sessionJSONLPath := filepath.Join(filepath.Dir(filePath), lastEntry.SessionID+".jsonl")
	jsonlToUse := filePath
	if _, err := os.Stat(sessionJSONLPath); err == nil {
		jsonlToUse = sessionJSONLPath
	}

	agent := &Agent{
		ID:          agentID,
		Slug:        lastEntry.Slug,
		SessionID:   lastEntry.SessionID,
		ProjectPath: lastEntry.CWD,
		ProjectName: projectName,
		GitBranch:   lastEntry.GitBranch,
		LastActive:  lastEntry.Timestamp,
		CurrentTask: "Loading...",
		TaskStatus:  "unknown",
		TokensUsed:  totalInput + totalOutput,
		TokensInput: totalInput,
		JSONLPath:   jsonlToUse,
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
