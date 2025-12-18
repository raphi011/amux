package watcher

import (
	"context"
	"os"
	"path/filepath"
	"time"

	"github.com/fsnotify/fsnotify"
	tea "github.com/charmbracelet/bubbletea"
)

// FileWatcher manages file system watching
type FileWatcher struct {
	watcher  *fsnotify.Watcher
	debounce map[string]*time.Timer
}

// FileChangedMsg signals a file system change
type FileChangedMsg struct {
	Path string
}

// NewWatcher creates a new file watcher for the specified directories
func NewWatcher(dirs []string) (*FileWatcher, error) {
	w, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	fw := &FileWatcher{
		watcher:  w,
		debounce: make(map[string]*time.Timer),
	}

	// Recursively watch directories
	for _, dir := range dirs {
		if err := fw.addRecursive(dir); err != nil {
			// Log error but continue with other directories
			continue
		}
	}

	return fw, nil
}

// addRecursive adds a directory and its subdirectories to the watcher
// Limited to 2 levels deep to avoid too many open files
func (fw *FileWatcher) addRecursive(dir string) error {
	// Check if directory exists
	if _, err := os.Stat(dir); os.IsNotExist(err) {
		return err
	}

	// Add the directory itself
	if err := fw.watcher.Add(dir); err != nil {
		return err
	}

	// Walk subdirectories (limit to 2 levels deep)
	err := filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil // Skip errors
		}

		if !info.IsDir() {
			return nil
		}

		// Calculate depth
		rel, _ := filepath.Rel(dir, path)
		depth := len(filepath.SplitList(rel))
		if depth > 2 {
			return filepath.SkipDir
		}

		// Add directory to watcher
		return fw.watcher.Add(path)
	})

	return err
}

// Start begins watching for file changes and returns a bubbletea command
func (fw *FileWatcher) Start(ctx context.Context) tea.Cmd {
	return func() tea.Msg {
		for {
			select {
			case <-ctx.Done():
				fw.watcher.Close()
				return nil

			case event, ok := <-fw.watcher.Events:
				if !ok {
					return nil
				}

				// Only watch for write and create events
				if event.Op&fsnotify.Write == 0 && event.Op&fsnotify.Create == 0 {
					continue
				}

				// Handle new directories
				if event.Op&fsnotify.Create != 0 {
					if info, err := os.Stat(event.Name); err == nil && info.IsDir() {
						fw.addRecursive(event.Name)
					}
				}

				// Simple debounce - wait briefly then return
				time.Sleep(100 * time.Millisecond)
				return FileChangedMsg{Path: event.Name}

			case err, ok := <-fw.watcher.Errors:
				if !ok {
					return nil
				}
				// Log error but continue watching
				_ = err
			}
		}
	}
}

// Close stops the watcher
func (fw *FileWatcher) Close() error {
	if fw.watcher != nil {
		return fw.watcher.Close()
	}
	return nil
}
