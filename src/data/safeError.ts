let errorSequence = 0;

export function createCorrelationId(): string {
  errorSequence += 1;
  const random = globalThis.crypto?.randomUUID?.().slice(0, 8) ?? Date.now().toString(36);
  return `S16-${random}-${errorSequence}`;
}

// Backend failures can include paths or other private details. Keep those out of UI copy.
export function safeErrorMessage(action: string, correlationId = createCorrelationId()): string {
  return `Не удалось ${action}. Повторите попытку. Если проблема останется, сообщите ID: ${correlationId}.`;
}
