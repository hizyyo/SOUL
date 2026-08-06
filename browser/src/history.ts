import { collapseText, type CollapseResult } from './compose';

export function collapsibleUserText(text: string, isUserMessage: boolean): CollapseResult | null {
  if (!isUserMessage) {
    return null;
  }
  const result = collapseText(text);
  return result && result.rest.trim() === '' ? result : null;
}
