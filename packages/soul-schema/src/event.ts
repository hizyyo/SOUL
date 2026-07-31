import { z } from 'zod';
import { SoulEntityType } from './entity';

export const EventOperation = z.enum([
  'soul.created',
  'soul.preview_confirmed',
  'soul.preview_revoked',
  'soul.activated',
  'candidate.proposed',
  'entity.activated',
  'entity.updated',
  'entity.superseded',
  'entity.rejected',
  'entity.deleted',
]);

export type EventOperation = z.infer<typeof EventOperation>;

export const SoulEvent = z.object({
  eventId: z.string(),
  soulId: z.string(),
  deviceId: z.string(),
  actor: z.enum(['user', 'importer', 'agent', 'system']),
  hlc: z.string(),
  operation: EventOperation,
  entityType: SoulEntityType,
  entityId: z.string(),
  payload: z.unknown(),
  provenanceIds: z.array(z.string()),
  previousEventHash: z.string().nullable(),
  contentHash: z.string(),
  signature: z.string(),
  createdAt: z.string(),
});

export type SoulEvent = z.infer<typeof SoulEvent>;
