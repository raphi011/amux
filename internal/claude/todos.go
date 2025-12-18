package claude

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
)

// TodoItem represents a single todo item
type TodoItem struct {
	Content    string `json:"content"`
	Status     string `json:"status"`
	ActiveForm string `json:"activeForm"`
}

// FindTodoFile searches for a todo file matching the agent pattern
func FindTodoFile(agentID string) (string, error) {
	todosDir, err := GetTodosDir()
	if err != nil {
		return "", err
	}

	// List all todo files
	files, err := os.ReadDir(todosDir)
	if err != nil {
		return "", err
	}

	// Look for files containing the agent ID
	for _, file := range files {
		if strings.Contains(file.Name(), agentID) && strings.HasSuffix(file.Name(), ".json") {
			return filepath.Join(todosDir, file.Name()), nil
		}
	}

	return "", nil
}

// ParseTodoFile reads and parses a todo JSON file
func ParseTodoFile(filePath string) ([]TodoItem, error) {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return nil, err
	}

	var todos []TodoItem
	if err := json.Unmarshal(data, &todos); err != nil {
		return nil, err
	}

	return todos, nil
}

// GetCurrentTask returns the current task from a todo list
func GetCurrentTask(todos []TodoItem) (string, string) {
	// Look for in_progress tasks first
	for _, todo := range todos {
		if todo.Status == "in_progress" {
			return todo.Content, todo.Status
		}
	}

	// Then look for pending tasks
	for _, todo := range todos {
		if todo.Status == "pending" {
			return todo.Content, todo.Status
		}
	}

	// Finally return first completed task if no active ones
	for _, todo := range todos {
		if todo.Status == "completed" {
			return todo.Content, todo.Status
		}
	}

	return "No tasks", "unknown"
}
