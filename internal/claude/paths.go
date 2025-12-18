package claude

import (
	"os"
	"path/filepath"
)

// GetClaudeDir returns the path to the .claude directory
func GetClaudeDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".claude"), nil
}

// GetProjectsDir returns the path to the projects directory
func GetProjectsDir() (string, error) {
	claudeDir, err := GetClaudeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(claudeDir, "projects"), nil
}

// GetSessionEnvDir returns the path to the session-env directory
func GetSessionEnvDir() (string, error) {
	claudeDir, err := GetClaudeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(claudeDir, "session-env"), nil
}

// GetTodosDir returns the path to the todos directory
func GetTodosDir() (string, error) {
	claudeDir, err := GetClaudeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(claudeDir, "todos"), nil
}
