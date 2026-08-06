import type { HealthReport, Page, PageElement, SiteAdapter } from './types';

interface ProbeExpectations {
  inputTag: string;
  mountTag: string;
}

export function probeAdapter(
  adapter: SiteAdapter,
  page: Page,
  expectations: ProbeExpectations,
): HealthReport {
  const input = page.querySelector(adapter.inputSelector);
  const send = page.querySelector(adapter.sendSelector);
  const mount = page.querySelector(adapter.mountSelector);
  const missing = [
    ...problem(input, adapter.inputSelector, expectations.inputTag, 'editable'),
    ...problem(
      send,
      adapter.sendSelector,
      'BUTTON',
      input?.value.trim() ? 'enabled' : 'connected',
    ),
    ...problem(mount, adapter.mountSelector, expectations.mountTag, 'connected'),
  ];
  return {
    status: missing.length === 0 ? 'ok' : 'failed',
    missing,
    checked: [adapter.inputSelector, adapter.sendSelector, adapter.mountSelector],
    at: Date.now(),
  };
}

function problem(
  element: PageElement | null,
  selector: string,
  tagName: string,
  semantic: 'editable' | 'enabled' | 'connected',
): string[] {
  if (!element) {
    return [selector];
  }
  if (element.tagName !== tagName || !element.connected) {
    return [`${selector} (${tagName.toLowerCase()}, connected)`];
  }
  if (semantic === 'editable' && !element.editable) {
    return [`${selector} (editable)`];
  }
  if (semantic === 'enabled' && !element.enabled) {
    return [`${selector} (enabled)`];
  }
  return [];
}
