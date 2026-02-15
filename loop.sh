#!/bin/bash
# Usage: ./loop.sh [max_iterations]
# Examples:
#   ./loop.sh              # Build mode, unlimited iterations
#   ./loop.sh 20           # Build mode, max 20 iterations

# Parse arguments
if [[ "$1" =~ ^[0-9]+$ ]]; then
  # Build mode with max iterations
  MAX_ITERATIONS=$1
else
  # Build mode, unlimited (no arguments or invalid input)
  MAX_ITERATIONS=0
fi

MODE="build"
PROMPT_FILE="PROMPT.md"

ITERATION=0
CURRENT_BRANCH=$(git branch --show-current)

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Mode:   $MODE"
echo "Prompt: $PROMPT_FILE"
echo "Branch: $CURRENT_BRANCH"
[ $MAX_ITERATIONS -gt 0 ] && echo "Max:    $MAX_ITERATIONS iterations"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Verify prompt file exists
if [ ! -f "$PROMPT_FILE" ]; then
  echo "Error: $PROMPT_FILE not found"
  exit 1
fi

while true; do
  if [ $MAX_ITERATIONS -gt 0 ] && [ $ITERATION -ge $MAX_ITERATIONS ]; then
    echo "Reached max iterations: $MAX_ITERATIONS"
    break
  fi

  # cat "$PROMPT_FILE" | opencode run --model opencode/kimi-k2.5-free
  cat "$PROMPT_FILE" | opencode run --model anthropic/claude-opus-4-6

  # Push changes after each iteration
  git push origin "$CURRENT_BRANCH" || {
    echo "Failed to push. Creating remote branch..."
    git push -u origin "$CURRENT_BRANCH"
  }

  ITERATION=$((ITERATION + 1))
  echo -e "\n\n======================== LOOP $ITERATION ========================\n"
done
