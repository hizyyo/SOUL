import type { CalibrationAnswer, CalibrationQuestion } from './calibration';
import { buildEntityData, type EntityData } from './review';

export type P0EntityType = 'preference' | 'decision' | 'boundary' | 'goal' | 'fact';

const P0_TYPES: readonly P0EntityType[] = ['preference', 'decision', 'boundary', 'goal', 'fact'];

export interface CompiledItem {
  questionId: string;
  type: P0EntityType;
  data: EntityData;
  disputed: boolean;
}

function nonEmpty(value: string | string[] | undefined): boolean {
  if (typeof value === 'string') return value.trim().length > 0;
  return Array.isArray(value) && value.length > 0;
}

/** Детерминированное правило спора: если пользователь сказал «ничего не запрещено»
 *  (bound_2), но при этом выбрал темы, которые ИИ не должен решать сам (bound_1),
 *  оба результата помечаются спорными и требуют индивидуального подтверждения. */
function computeDispute(
  answers: CalibrationAnswer[],
  questionId: string,
  type: P0EntityType,
): boolean {
  if (type !== 'boundary') return false;
  const bound2 = answers.find((a) => a.questionId === 'bound_2');
  const bound1 = answers.find((a) => a.questionId === 'bound_1');
  const nothingOffLimits =
    bound2 &&
    (typeof bound2.value === 'string' ? bound2.value : '').includes('Nothing is off-limits');
  const hasBound1Topics = bound1 && nonEmpty(bound1.value);
  if (!nothingOffLimits || !hasBound1Topics) return false;
  return questionId === 'bound_1' || questionId === 'bound_2';
}

/** Детерминированный компилятор ответов калибровки в типизированные сущности P0.
 *  Чистая синхронная функция: без сети, без модели, без времени и случайности —
 *  одинаковый ввод всегда даёт одинаковый вывод (нулевое использование токенов). */
export function compileAnswers(
  answers: CalibrationAnswer[],
  questions: CalibrationQuestion[],
): CompiledItem[] {
  const byId = new Map(questions.map((q) => [q.id, q]));
  const compiled: CompiledItem[] = [];
  for (const answer of answers) {
    const question = byId.get(answer.questionId);
    if (!question) continue;
    if (!P0_TYPES.includes(question.category)) continue;
    const data = buildEntityData(question, answer);
    if (!data) continue;
    const disputed = computeDispute(answers, question.id, question.category);
    if (disputed) data.disputed = true;
    compiled.push({
      questionId: question.id,
      type: question.category,
      data,
      disputed,
    });
  }
  return compiled;
}
