#!/bin/bash

# Test script with lorem ipsum to simulate long Claude messages

CLAUDE_DIR="$HOME/.claude/projects"
JSONL_FILE=$(find "$CLAUDE_DIR" -name "*.jsonl" -type f | head -1)

if [ -z "$JSONL_FILE" ]; then
    echo "No JSONL file found in $CLAUDE_DIR"
    exit 1
fi

echo "Writing lorem ipsum messages to: $JSONL_FILE"
echo "Watch amux - should auto-scroll at the top!"
echo ""

LOREM=(
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua."
    "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
    "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur."
    "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."
    "Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium."
    "Totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo."
    "Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores."
    "Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit."
)

for i in {1..10}; do
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")

    # Pick a random lorem ipsum paragraph
    RANDOM_TEXT="${LOREM[$((RANDOM % ${#LOREM[@]}))]}"

    # User message
    cat >> "$JSONL_FILE" << EOF
{"sessionId":"test","agentId":"test","slug":"test","timestamp":"$TIMESTAMP","message":{"role":"user","content":"Message $i: $RANDOM_TEXT"},"cwd":"$(pwd)","gitBranch":"main"}
EOF

    echo "[$i/10] User message at $(date +%H:%M:%S)"
    sleep 1

    # Assistant response with multiple paragraphs
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")
    RESPONSE="${LOREM[0]}\n\n${LOREM[1]}\n\n${LOREM[2]}\n\nThis is response $i with timestamp $(date +%H:%M:%S)"

    cat >> "$JSONL_FILE" << EOF
{"sessionId":"test","agentId":"test","slug":"test","timestamp":"$TIMESTAMP","message":{"role":"assistant","content":"$RESPONSE"},"cwd":"$(pwd)","gitBranch":"main"}
EOF

    echo "[$i/10] Assistant response at $(date +%H:%M:%S)"
    sleep 1
done

echo ""
echo "✓ Done! Check amux - should have scrolled to show all 10 new message pairs."
