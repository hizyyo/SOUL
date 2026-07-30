import { z } from 'zod';

export const SoulEntityType = z.enum([
  'preference',
  'decision',
  'boundary',
  'goal',
  'fact',
]);

export type SoulEntityType = z.infer<typeof SoulEntityType>;

export const EntityStatus = z.enum([
  'candidate',
  'active',
  'disputed',
  'superseded',
  'expired',
  'rejected',
  'deleted',
]);

export type EntityStatus = z.infer<typeof EntityStatus>;

export const Sensitivity = z.enum([
  'public',
  'internal',
  'private',
  'sensitive',
  'restricted',
]);

export type Sensitivity = z.infer<typeof Sensitivity>;

export const Stability = z.enum([
  'ephemeral',
  'situational',
  'stable',
]);

export type Stability = z.infer<typeof Stability>;

const EntityScope = z.object({
  domains: z.array(z.string()),
  projects: z.array(z.string()),
  people: z.array(z.string()),
  channels: z.array(z.string()),
});

export const SoulEntity = z.object({
  id: z.string(),
  type: SoulEntityType,
  namespace: z.string(),
  subject: z.string(),
  status: EntityStatus,
  scope: EntityScope,
  confidence: z.number().min(0).max(1),
  importance: z.number().min(0).max(1),
  sensitivity: Sensitivity,
  stability: Stability,
  validFrom: z.string().nullable(),
  validUntil: z.string().nullable(),
  evidenceIds: z.array(z.string()),
  supersedes: z.array(z.string()),
  conflictsWith: z.array(z.string()),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type SoulEntity = z.infer<typeof SoulEntity>;

export const PreferenceEntity = SoulEntity.extend({
  type: z.literal('preference'),
  value: z.string(),
  strength: z.number().min(0).max(1),
  exceptions: z.array(z.string()),
  alternatives: z.array(z.string()),
});

export type PreferenceEntity = z.infer<typeof PreferenceEntity>;

export const DecisionEntity = SoulEntity.extend({
  type: z.literal('decision'),
  question: z.string(),
  options: z.array(z.string()),
  selected: z.string(),
  reasons: z.array(z.string()),
  rejectedReasons: z.array(z.string()),
  conditionsThatWouldChangeDecision: z.array(z.string()),
  outcome: z.string().nullable(),
});

export type DecisionEntity = z.infer<typeof DecisionEntity>;

export const BoundaryEntity = SoulEntity.extend({
  type: z.literal('boundary'),
  hardness: z.enum(['soft', 'hard', 'immutable']),
  actionKinds: z.array(z.string()),
  effect: z.enum(['deny', 'require_confirmation', 'redact']),
});

export type BoundaryEntity = z.infer<typeof BoundaryEntity>;

export const GoalEntity = SoulEntity.extend({
  type: z.literal('goal'),
  description: z.string(),
  priority: z.number().min(0).max(1),
  deadline: z.string().nullable(),
  progress: z.number().min(0).max(1),
});

export type GoalEntity = z.infer<typeof GoalEntity>;

export const FactEntity = SoulEntity.extend({
  type: z.literal('fact'),
  value: z.unknown(),
  category: z.string(),
});

export type FactEntity = z.infer<typeof FactEntity>;

export type AnyEntity =
  | PreferenceEntity
  | DecisionEntity
  | BoundaryEntity
  | GoalEntity
  | FactEntity;

export const AnyEntitySchema = z.discriminatedUnion('type', [
  PreferenceEntity,
  DecisionEntity,
  BoundaryEntity,
  GoalEntity,
  FactEntity,
]);
