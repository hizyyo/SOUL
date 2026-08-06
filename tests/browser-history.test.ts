import { describe, expect, it } from 'vitest';
import { composeMessage } from '../browser/src/compose';
import { collapsibleUserText } from '../browser/src/history';

const pack = 'SOUL CONTEXT\nentities: 1\n[item] preference';

describe('browser history collapse', () => {
  it('collapses only an exact user-authored companion message', () => {
    const text = composeMessage('question', pack).text;
    expect(collapsibleUserText(text, true)?.userText).toBe('question');
    expect(collapsibleUserText(text, false)).toBeNull();
    expect(collapsibleUserText(`${text}\nassistant suffix`, true)).toBeNull();
  });
});
