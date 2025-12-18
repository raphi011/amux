package claude

import (
	"bufio"
	"encoding/json"
	"os"
	"time"
)

// JSONLEntry represents a single line in a JSONL file
type JSONLEntry struct {
	SessionID string    `json:"sessionId"`
	AgentID   string    `json:"agentId"`
	Slug      string    `json:"slug"`
	Timestamp time.Time `json:"timestamp"`
	Message   struct {
		Role    string `json:"role"`
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text,omitempty"`
		} `json:"content"`
	} `json:"message"`
	Usage struct {
		InputTokens              int `json:"input_tokens"`
		OutputTokens             int `json:"output_tokens"`
		CacheCreationInputTokens int `json:"cache_creation_input_tokens"`
		CacheReadInputTokens     int `json:"cache_read_input_tokens"`
	} `json:"usage"`
	CWD string `json:"cwd"`
}

// ParseJSONL reads a JSONL file and returns all entries
func ParseJSONL(filePath string) ([]JSONLEntry, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var entries []JSONLEntry
	scanner := bufio.NewScanner(file)

	// Increase buffer size for large lines
	buf := make([]byte, 0, 64*1024)
	scanner.Buffer(buf, 10*1024*1024)

	for scanner.Scan() {
		var entry JSONLEntry
		if err := json.Unmarshal(scanner.Bytes(), &entry); err != nil {
			// Skip malformed lines
			continue
		}
		entries = append(entries, entry)
	}

	if err := scanner.Err(); err != nil {
		return nil, err
	}

	return entries, nil
}

// GetLastEntry returns the last entry from a JSONL file
func GetLastEntry(filePath string) (*JSONLEntry, error) {
	entries, err := ParseJSONL(filePath)
	if err != nil {
		return nil, err
	}

	if len(entries) == 0 {
		return nil, nil
	}

	return &entries[len(entries)-1], nil
}
