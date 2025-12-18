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
		Role    string          `json:"role"`
		Content json.RawMessage `json:"content"` // Can be string or array
		Usage   struct {
			InputTokens              int `json:"input_tokens"`
			OutputTokens             int `json:"output_tokens"`
			CacheCreationInputTokens int `json:"cache_creation_input_tokens"`
			CacheReadInputTokens     int `json:"cache_read_input_tokens"`
		} `json:"usage"`
	} `json:"message"`
	CWD       string `json:"cwd"`
	GitBranch string `json:"gitBranch"`
}

// ContentItem represents an item in the content array
type ContentItem struct {
	Type string `json:"type"`
	Text string `json:"text,omitempty"`
}

// GetContentText extracts text from the Content field (handles both string and array)
func (e *JSONLEntry) GetContentText() string {
	// Try to unmarshal as string first
	var str string
	if err := json.Unmarshal(e.Message.Content, &str); err == nil {
		return str
	}

	// Try to unmarshal as array of ContentItem
	var items []ContentItem
	if err := json.Unmarshal(e.Message.Content, &items); err == nil {
		var text string
		for _, item := range items {
			if item.Type == "text" && item.Text != "" {
				text += item.Text
			}
		}
		return text
	}

	return ""
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
