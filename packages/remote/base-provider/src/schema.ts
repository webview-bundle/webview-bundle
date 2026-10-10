import { z } from 'zod/v4';

export const BundleUpdateSchema = z.object({
  name: z.string(),
  version: z.string(),
  integrity: z.string().optional(),
  metadata: z.record(z.string(), z.string()).optional(),
});
export type BundleUpdate = z.infer<typeof BundleUpdateSchema>;

export const UpdateDataSchema = z.object({
  id: z.uuidv7(),
  createdAt: z.iso.datetime(),
  runtimeVersion: z.int(),
  bundles: BundleUpdateSchema.array(),
  metadata: z.record(z.string(), z.string()).optional(),
});
export type UpdateData = z.infer<typeof UpdateDataSchema>;

export const UpdateSignatureSchema = z.object({
  id: z.string(),
  alg: z.string(),
  sig: z.string(),
});
export type UpdateSignature = z.infer<typeof UpdateSignatureSchema>;

export const UpdateSchema = z.object({
  data: UpdateDataSchema,
  etag: z.string().optional(),
  signatures: UpdateSignatureSchema.array(),
});
export type Update = z.infer<typeof UpdateSchema>;
