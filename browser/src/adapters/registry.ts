/**
 * Реестр site adapter-ов: выбор по происхождению страницы.
 */

import type { SiteAdapter } from './types';
import { chatgptAdapter } from './chatgpt';
import { geminiAdapter } from './gemini';
import { claudeAdapter } from './claude';

export const ADAPTERS: readonly SiteAdapter[] = [chatgptAdapter, geminiAdapter, claudeAdapter];

/** Адаптер для происхождения страницы или null (сайт не поддерживается). */
export function adapterForOrigin(origin: string): SiteAdapter | null {
  return ADAPTERS.find((adapter) => adapter.origin === origin) ?? null;
}

/** Версионированный идентификатор адаптера, например "chatgpt/v1". */
export function adapterVersionId(adapter: SiteAdapter): string {
  return `${adapter.id}/v${adapter.version}`;
}
