import { describe, it, expect } from 'vitest';
import {
  composeMessage,
  collapseText,
  chipLabel,
  hasSoulBlock,
  itemCountFromPack,
  SOUL_BLOCK_END,
  SOUL_BLOCK_START,
} from '../browser/src/compose';

const PACK_2 = `SOUL CONTEXT — entities: 2\n1. [preference] Не называть «ассистентом».`;

describe('itemCountFromPack', () => {
  it('извлекает число сущностей', () => {
    expect(itemCountFromPack(PACK_2)).toBe(2);
  });

  it('возвращает 0 для пакетов без сущностей', () => {
    expect(itemCountFromPack('')).toBe(0);
    expect(itemCountFromPack('no entities here')).toBe(0);
    expect(itemCountFromPack('entities: 0')).toBe(0);
  });
});

describe('chipLabel', () => {
  it('согласует число с единственным числом', () => {
    expect(chipLabel(1)).toBe('SOUL context: 1 item');
    expect(chipLabel(2)).toBe('SOUL context: 2 items');
    expect(chipLabel(0)).toBe('SOUL context: 0 items');
  });
});

describe('composeMessage', () => {
  it('добавляет технический блок после черновика', () => {
    const composed = composeMessage('Привет!', PACK_2);
    expect(composed.text).toBe(`Привет!\n\n${SOUL_BLOCK_START}\n${PACK_2.trim()}\n${SOUL_BLOCK_END}`);
    expect(composed.entityCount).toBe(2);
    expect(composed.block).toContain('entities: 2');
    expect(composed.label).toBe('SOUL context: 2 items');
  });

  it('не оставляет висячих переносов при пустом черновике', () => {
    const composed = composeMessage('   ', PACK_2);
    expect(composed.text.startsWith('\n')).toBe(false);
    expect(composed.text.startsWith(SOUL_BLOCK_START)).toBe(true);
  });

  it('сохраняет черновик без изменений внутри текста', () => {
    const draft = 'Вопрос про отпуск';
    const composed = composeMessage(draft, PACK_2);
    expect(composed.text).toContain(draft);
    expect(composed.text.indexOf(draft)).toBeLessThan(composed.text.indexOf(SOUL_BLOCK_START));
  });
});

describe('collapseText', () => {
  const full = composeMessage('Вопрос', PACK_2).text;

  it('выделяет текст пользователя, блок и остаток', () => {
    const result = collapseText(full);
    expect(result).not.toBeNull();
    if (result) {
      expect(result.userText).toBe('Вопрос');
      expect(result.block.startsWith(SOUL_BLOCK_START)).toBe(true);
      expect(result.block.endsWith(SOUL_BLOCK_END)).toBe(true);
      expect(result.count).toBe(2);
    }
  });

  it('возвращает null без блока или без сущностей', () => {
    expect(collapseText('обычное сообщение')).toBeNull();
    const emptyBlock = `${SOUL_BLOCK_START}\nentities: 0\n${SOUL_BLOCK_END}`;
    expect(collapseText(emptyBlock)).toBeNull();
  });

  it('возвращает null при незакрытом блоке', () => {
    expect(collapseText(`${SOUL_BLOCK_START} без закрытия`)).toBeNull();
  });

  it('сохраняет остаток после блока', () => {
    const text = `${SOUL_BLOCK_START}\n${PACK_2}\n${SOUL_BLOCK_END} ещё текст`;
    const result = collapseText(text);
    expect(result).not.toBeNull();
    if (result) {
      expect(result.rest).toBe(' ещё текст');
      expect(result.userText).toBe('');
    }
  });
});

describe('hasSoulBlock', () => {
  it('обнаруживает пару маркеров', () => {
    expect(hasSoulBlock(`${SOUL_BLOCK_START} x ${SOUL_BLOCK_END}`)).toBe(true);
    expect(hasSoulBlock(SOUL_BLOCK_START)).toBe(false);
    expect(hasSoulBlock('нет')).toBe(false);
  });
});
