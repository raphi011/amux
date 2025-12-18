package claude

// CalculateTotalTokens sums up all token usage from JSONL entries
func CalculateTotalTokens(entries []JSONLEntry) (totalInput, totalOutput int) {
	for _, entry := range entries {
		totalInput += entry.Usage.InputTokens
		totalOutput += entry.Usage.OutputTokens
	}
	return totalInput, totalOutput
}
