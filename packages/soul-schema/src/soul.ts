import { z } from 'zod';

export const SoulManifest = z.object({
  soulId: z.string(),
  displayName: z.string(),
  formatVersion: z.string(),
  schemaVersion: z.string(),
  createdAt: z.string(),
  headEventHash: z.string().nullable(),
  entityCount: z.number(),
  deviceId: z.string(),
});

export type SoulManifest = z.infer<typeof SoulManifest>;
