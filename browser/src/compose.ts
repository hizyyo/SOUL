/**
 * Композиция сообщения: пользовательский черновик + технический блок
 * SOUL context. Блок оформлен как раскрываемая секция для истории чата.
 */

export const SOUL_BLOCK_START = '[SOUL context]';
export const SOUL_BLOCK_END = '[/SOUL context]';

/** Число сущностей из пакета (компилятор пишет "entities: N"). */
export function itemCountFromPack(pack: string): number {
  const match = /entities:\s*(\d+)/.exec(pack);
  if (!match) {
    return 0;
  }
  const count = Number(match[1]);
  return Number.isSafeInteger(count) && count > 0 ? count : 0;
}

/** Подпись чипа в истории: "SOUL context: N items". */
export function chipLabel(count: number): string {
  return `SOUL context: ${count} item${count === 1 ? '' : 's'}`;
}

export interface ComposedMessage {
  /** Полный текст для вставки в поле ввода. */
  text: string;
  /** Только технический блок (без пользовательского текста). */
  block: string;
  entityCount: number;
  label: string;
}

/** Собирает текст сообщения: черновик + пустая строка + технический блок. */
export function composeMessage(draft: string, pack: string): ComposedMessage {
  const entityCount = itemCountFromPack(pack);
  const block = `${SOUL_BLOCK_START}\n${pack.trim()}\n${SOUL_BLOCK_END}`;
  const trimmedDraft = draft.trim();
  const text = trimmedDraft === '' ? block : `${trimmedDraft}\n\n${block}`;
  return { text, block, entityCount, label: chipLabel(entityCount) };
}

export interface CollapseResult {
  /** Часть текста до технического блока (текст пользователя). */
  userText: string;
  /** Технический блок целиком (сохраняется внутри чипа). */
  block: string;
  /** Часть текста после технического блока. */
  rest: string;
  count: number;
}

/**
 * Разбирает текст сообщения из истории: выделяет технический блок
 * между маркерами. Возвращает null, если блока нет или он не содержит
 * сущностей (тогда сворачивать нельзя).
 */
export function collapseText(text: string): CollapseResult | null {
  const startIdx = text.indexOf(SOUL_BLOCK_START);
  if (startIdx < 0) {
    return null;
  }
  const endIdx = text.indexOf(SOUL_BLOCK_END, startIdx);
  if (endIdx < 0) {
    return null;
  }
  const endAt = endIdx + SOUL_BLOCK_END.length;
  const block = text.slice(startIdx, endAt);
  const count = itemCountFromPack(block);
  if (count <= 0) {
    return null;
  }
  return {
    userText: text.slice(0, startIdx).trimEnd(),
    block,
    rest: text.slice(endAt),
    count,
  };
}

/** Есть ли в тексте маркеры технического блока. */
export function hasSoulBlock(text: string): boolean {
  return text.includes(SOUL_BLOCK_START) && text.includes(SOUL_BLOCK_END);
}
