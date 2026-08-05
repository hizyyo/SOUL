import { describe, expect, it } from 'vitest';
import { DEMO_ENTITIES, DEMO_NOTICE, INVESTOR_DEMO_STEPS } from '../src/data/demo';

describe('demo fixtures', () => {
  it('are explicitly synthetic and contain a complete 55-second story', () => {
    expect(DEMO_NOTICE).toMatch(/синтетическ/i);
    expect(DEMO_NOTICE).toMatch(/не затрагиваются/i);
    expect(INVESTOR_DEMO_STEPS).toHaveLength(6);
    expect(INVESTOR_DEMO_STEPS.at(-1)?.at).toBe('48-55 c');
  });

  it('has static example data without a live identifier or external action', () => {
    expect(DEMO_ENTITIES).toHaveLength(3);
    expect(JSON.stringify(DEMO_ENTITIES)).not.toMatch(/soul_id|http|token|password/i);
  });
});
