#!/bin/bash

# Test script to simulate Claude writing messages
# This will append messages to demonstrate auto-scrolling

# Find an existing JSONL file or use a test one
CLAUDE_DIR="$HOME/.claude/projects"
JSONL_FILE=$(find "$CLAUDE_DIR" -name "*.jsonl" -type f | head -1)

if [ -z "$JSONL_FILE" ]; then
    echo "No JSONL file found in $CLAUDE_DIR"
    echo "Please run Claude first to create a session"
    exit 1
fi

echo "Writing test messages to: $JSONL_FILE"
echo "Watch amux to see auto-scrolling..."
echo ""

# Simulate 5 messages with delays
for i in {1..5}; do
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")

    # Create a user message
    cat >> "$JSONL_FILE" << EOF
{"sessionId":"test","agentId":"test","slug":"test","timestamp":"$TIMESTAMP","message":{"role":"user","content":"Test message $i - $(date +%H:%M:%S)"},"cwd":"$(pwd)","gitBranch":"main"}
EOF

    echo "Wrote message $i at $(date +%H:%M:%S)"
    sleep 2

    # Create an assistant response
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")
    cat >> "$JSONL_FILE" << EOF
{"sessionId":"test","agentId":"test","slug":"test","timestamp":"$TIMESTAMP","message":{"role":"assistant","content":"Response to message $i\nThis is a test response with multiple lines.\nLine 3 of the response.\nLine 4 of the response."},"cwd":"$(pwd)","gitBranch":"main"}
EOF

    echo "Wrote response $i at $(date +%H:%M:%S)"
    sleep 2
done

echo ""
echo "Done! Check amux - it should have auto-scrolled to show all new messages."
