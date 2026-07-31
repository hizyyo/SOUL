import type { CalibrationAnswer, CalibrationQuestion } from './calibration';

export type EntityKind = 'binary' | 'multiple' | 'text' | 'writing';
export type SensitivityLevel = 'public' | 'internal' | 'private' | 'sensitive' | 'restricted';

export interface EntityData {
  claim: string;
  evidence: string;
  source: 'calibration';
  questionId: string;
  kind: EntityKind;
  value?: string | string[];
  confidence: number;
  explicitness: number;
  sensitivity: SensitivityLevel;
  scope: {
    domains: string[];
    projects: string[];
    people: string[];
    channels: string[];
  };
  risk: boolean;
}

export interface ReviewEntity {
  id: string;
  entity_type: string;
  status: string;
  data: string;
  created_at: string;
}

const SENSITIVITY_PENALTY: Record<SensitivityLevel, number> = {
  public: 0,
  internal: 0.1,
  private: 0.4,
  sensitive: 0.8,
  restricted: 1,
};

function domainForQuestion(questionId: string): string {
  if (questionId.startsWith('goal_')) return 'goals';
  if (questionId.startsWith('bound_')) return 'boundaries';
  if (questionId.startsWith('dec_')) return 'decisions';
  if (questionId.startsWith('write_')) return 'writing';
  if (questionId.startsWith('text_')) return 'personal';
  return 'preferences';
}

const KIND_CONFIDENCE: Record<EntityKind, number> = {
  binary: 0.9,
  multiple: 0.85,
  text: 0.7,
  writing: 0.6,
};

const KIND_EXPLICITNESS: Record<EntityKind, number> = {
  binary: 1,
  multiple: 0.9,
  text: 0.8,
  writing: 0.7,
};

function maskEmail(text: string): string {
  return text.replace(
    /(?<![\p{L}\w.+-@])[\p{L}\w.+-]+@[\p{L}\w-]+\.[\p{L}\w.-]+(?![-\w.@\p{L}])/giu,
    '[email]',
  );
}

function maskLongNumbers(text: string): string {
  return text.replace(/\d[\d\s()-]*\d/g, (match) =>
    match.replace(/\D/g, '').length >= 16 ? '[number]' : match,
  );
}

function maskApiKeys(text: string): string {
  return text.replace(/\b(sk|pk|rk)-[A-Za-z0-9_-]{8,}\b/gi, '[key]');
}

function maskBearerTokens(text: string): string {
  return text.replace(/\bBearer\s+[A-Za-z0-9._~+/=-]{10,}\b/gi, '[token]');
}

function maskLongTokens(text: string): string {
  return text.replace(/\b[A-Za-z0-9_./+=-]{24,}\b/g, '[token]');
}

function maskSecretAssignments(text: string): string {
  return text.replace(
    /(?<=^|[\s"'([{-])(?:password|passwd|api[_-]?key|secret|[Пп]ароль)\s*[:=]\s*["']?[^\s,;]+/gi,
    '[secret]',
  );
}

function maskPhones(text: string): string {
  return text.replace(/\+\d[\d\s()-]{6,}\d\b|\b\d[\d\s()-]{8,}\d\b/g, (match) => {
    const isDateLike = /^\d{4}-\d{2}-\d{2}$/.test(match) || /^\d{2}\.\d{2}\.\d{4}$/.test(match);
    if (isDateLike) return match;
    const digits = match.replace(/\D/g, '').length;
    return digits >= 7 && digits <= 15 ? '[phone]' : match;
  });
}

export function maskText(text: string): string {
  if (!text) return text;
  let masked = text;
  masked = maskSecretAssignments(masked);
  masked = maskBearerTokens(masked);
  masked = maskApiKeys(masked);
  masked = maskEmail(masked);
  masked = maskPhones(masked);
  masked = maskLongNumbers(masked);
  masked = maskLongTokens(masked);
  return masked;
}

export function detectSensitivity(text: string, entityType: string): SensitivityLevel {
  const lower = text.toLowerCase();
  const secretish =
    /password|passwd|api[_-]?key|secret|пароль|ssn|иин|пенсион/i.test(lower) ||
    /\b(sk|pk|rk)-[A-Za-z0-9_-]{8,}\b/i.test(text) ||
    /Bearer\s+[A-Za-z0-9._~+/=-]{10,}/i.test(text);
  if (secretish) return 'sensitive';
  const personal =
    /(?<![\p{L}\w.+-@])[\p{L}\w.+-]+@[\p{L}\w-]+\.[\p{L}\w.-]+(?![-\w.@\p{L}])/iu.test(text) ||
    /\+\d[\d\s()-]{6,}\d\b/.test(text);
  if (personal) return 'private';
  if (entityType === 'boundary') return 'private';
  return 'internal';
}

export function buildEntityData(
  question: CalibrationQuestion,
  answer: CalibrationAnswer,
): EntityData | null {
  const raw = answer.value;
  const isEmpty =
    typeof raw === 'string' ? raw.trim().length === 0 : Array.isArray(raw) && raw.length === 0;

  if (question.type === 'text' || question.type === 'writing') {
    if (isEmpty) return null;
    const claim = typeof raw === 'string' ? raw.trim() : '';
    if (!claim) return null;
    return {
      claim,
      evidence: question.prompt,
      source: 'calibration',
      questionId: question.id,
      kind: question.type,
      confidence: KIND_CONFIDENCE[question.type],
      explicitness: KIND_EXPLICITNESS[question.type],
      sensitivity: detectSensitivity(claim, question.category),
      scope: {
        domains: [domainForQuestion(question.id)],
        projects: [],
        people: [],
        channels: [],
      },
      risk: question.category === 'boundary',
    };
  }

  const value = typeof raw === 'string' ? raw : Array.isArray(raw) ? raw : '';
  if (!value || (Array.isArray(value) && value.length === 0)) return null;
  const claim = `${question.prompt} — ${Array.isArray(value) ? value.join(', ') : value}`;

  return {
    claim,
    evidence: question.prompt,
    source: 'calibration',
    questionId: question.id,
    kind: question.type,
    value,
    confidence: KIND_CONFIDENCE[question.type],
    explicitness: KIND_EXPLICITNESS[question.type],
    sensitivity: detectSensitivity(claim, question.category),
    scope: {
      domains: [domainForQuestion(question.id)],
      projects: [],
      people: [],
      channels: [],
    },
    risk: question.category === 'boundary',
  };
}

export function parseEntityData(data: string): Partial<EntityData> {
  try {
    const parsed: unknown = JSON.parse(data);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Partial<EntityData>;
    }
  } catch {
    // fall through to empty
  }
  return {};
}

export function claimOf(entity: ReviewEntity): string {
  const data = parseEntityData(entity.data);
  if (data.claim && typeof data.claim === 'string') return data.claim;
  return entity.data;
}

export function computeActivationScore(data: Partial<EntityData>): number {
  const explicitness = Math.min(1, Math.max(0, data.explicitness ?? 0.7));
  const sensitivity: SensitivityLevel = data.sensitivity ?? 'internal';
  const penalty = SENSITIVITY_PENALTY[sensitivity] ?? 0.1;
  return (
    0.3 * explicitness +
    0.2 * 1 + // source trust: explicit calibration
    0.15 * 0 + // repetition: none in calibration
    0.15 * 1 + // extraction confidence: deterministic
    0.1 * 0.5 - // future utility default
    0.1 * penalty -
    0.2 * 0 // contradiction risk: none detected
  );
}

export function rankCandidates(entities: ReviewEntity[]): ReviewEntity[] {
  const riskTier = (e: ReviewEntity): number => {
    const data = parseEntityData(e.data);
    if (e.entity_type === 'boundary' || data.risk === true) return 2;
    if (data.sensitivity === 'sensitive' || data.sensitivity === 'restricted') return 1;
    return 0;
  };
  return [...entities].sort((a, b) => {
    const aRisk = riskTier(a);
    const bRisk = riskTier(b);
    if (aRisk !== bRisk) return bRisk - aRisk;
    const scoreDiff =
      computeActivationScore(parseEntityData(b.data)) -
      computeActivationScore(parseEntityData(a.data));
    if (scoreDiff !== 0) return scoreDiff;
    return b.created_at.localeCompare(a.created_at);
  });
}

export function formatSourceDate(createdAt: string): string {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return createdAt;
  return date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}
