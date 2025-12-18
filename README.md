# Claude Manager

A TUI (Terminal User Interface) application for monitoring and managing multiple Claude Code agent instances.

## Features

- **Global Agent Monitoring**: Automatically detects all running Claude Code agents across all projects
- **Real-time Status**: Shows current task, status, and last activity for each agent
- **Visual Indicators**: Color-coded active/inactive status with easy-to-read formatting
- **Keyboard Navigation**: Vim-style shortcuts for efficient navigation
- **Auto-refresh**: Updates agent information every 2 seconds
- **Responsive UI**: Handles large numbers of agents smoothly

## Installation

### Prerequisites

- Go 1.21 or later
- Claude Code CLI installed

### Build from source

```bash
cd claude-manager
go mod download
go build -o claude-manager
```

### Install globally

```bash
go install
```

## Usage

Simply run the application:

```bash
./claude-manager
```

Or if installed globally:

```bash
claude-manager
```

### Keyboard Controls

- `↑` / `k` - Move cursor up
- `↓` / `j` - Move cursor down
- `g` - Jump to top of list
- `G` - Jump to bottom of list
- `r` - Refresh agent list manually
- `x` / `delete` - Kill selected agent (coming soon)
- `q` / `Esc` / `Ctrl+C` - Quit

## How It Works

Claude Manager monitors your `~/.claude/` directory structure to detect and track running agents:

1. **Agent Detection**: Scans `~/.claude/projects/` for agent JSONL files
2. **Status Tracking**: Parses agent transcripts to determine last activity
3. **Task Information**: Reads todo files from `~/.claude/todos/` to show current tasks
4. **Active Detection**: Marks agents as active if they've been active in the last 5 minutes

## Display Information

For each agent, the manager displays:

- **Indicator**: Green circle (●) for active, gray circle (○) for inactive
- **Name**: Human-readable slug (e.g., "typed-doodling-meerkat") or agent ID
- **Agent ID**: Short identifier (e.g., "a1a0698") shown in parentheses if slug exists
- **Project**: Working directory path
- **Current Task**: Task description with status badge
  - `[IN PROGRESS]` - Currently executing (green)
  - `[PENDING]` - Waiting to be executed (yellow)
  - `[COMPLETED]` - Finished (blue)
- **Last Active**: Human-readable time since last activity

## Architecture

```
claude-manager/
├── main.go                     # Entry point
├── internal/
│   ├── agent/
│   │   ├── agent.go           # Agent data structure
│   │   └── scanner.go         # Agent detection logic
│   ├── claude/
│   │   ├── paths.go           # Claude directory paths
│   │   ├── jsonl.go           # JSONL parser
│   │   └── todos.go           # Todo file parser
│   └── ui/
│       ├── model.go           # Bubbletea model
│       ├── update.go          # Update logic (with responsive fixes)
│       ├── view.go            # Rendering
│       └── styles.go          # Lipgloss styling
```

## Dependencies

- [Bubbletea](https://github.com/charmbracelet/bubbletea) - TUI framework
- [Lipgloss](https://github.com/charmbracelet/lipgloss) - Terminal styling

## Performance Notes

- Handles hundreds of agents efficiently
- Async agent loading prevents UI blocking
- Auto-refresh respects loading state to prevent multiple concurrent scans
- Keyboard input is always responsive, even during loading

## Future Enhancements

- Agent killing functionality
- Detail view with full message history
- Real-time log tailing
- Filter by project or status
- Agent statistics (token usage, runtime)
- Export session data
- Resume/attach to agent session

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

MIT License
