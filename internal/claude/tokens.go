package claude

// CalculateTotalTokens sums up all token usage from JSONL entries
func CalculateTotalTokens(entries []JSONLEntry) (totalInput, totalOutput int) {
	for _, entry := range entries {
		totalInput += entry.Message.Usage.InputTokens
		totalOutput += entry.Message.Usage.OutputTokens
	}
	return totalInput, totalOutput
}
