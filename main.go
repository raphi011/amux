package main

import (
	"fmt"
	"os"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/raphaelgruber/amux/internal/ui"
)

func main() {
	// Add panic recovery for better error messages
	defer func() {
		if r := recover(); r != nil {
			fmt.Fprintf(os.Stderr, "Fatal error: %v\n", r)
			os.Exit(1)
		}
	}()

	// Create the Bubbletea program
	p := tea.NewProgram(ui.NewModel(), tea.WithAltScreen())

	// Run the program
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error running program: %v\n", err)
		fmt.Fprintf(os.Stderr, "\nNote: This program requires a terminal that supports TUI (Terminal User Interface).\n")
		fmt.Fprintf(os.Stderr, "If you're running in a non-interactive environment, the program cannot start.\n")
		os.Exit(1)
	}
}
