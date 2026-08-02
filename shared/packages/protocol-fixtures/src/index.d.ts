import type { MessageSchema } from "@ym-connect/protocol";

export interface FixtureManifestEntry {
  readonly name: string;
  readonly type_name: string;
  readonly binary: string;
  readonly json: string;
  readonly binary_size: number;
  readonly binary_sha256: string;
  readonly json_sha256: string;
}
export interface FixtureManifest {
  readonly schema_version: 1;
  readonly descriptor_sha256: string;
  readonly fixtures: readonly FixtureManifestEntry[];
}
export interface LoadedFixture<T = unknown> {
  readonly entry: FixtureManifestEntry;
  readonly schema: MessageSchema<T>;
  readonly bytes: Uint8Array;
  readonly json: unknown;
}
export interface VerifiedFixture {
  readonly name: string;
  readonly typeName: string;
  readonly byteLength: number;
  readonly sha256: string;
}
export declare const fixtureDirectory: string;
export declare const manifestPath: string;
export declare function loadFixtureManifest(): Promise<FixtureManifest>;
export declare function loadFixture<T = unknown>(name: string): Promise<LoadedFixture<T>>;
export declare function verifyFixture(name: string): Promise<VerifiedFixture>;
export declare function verifyAllFixtures(): Promise<VerifiedFixture[]>;
export declare function writeCanonicalFixtures(options?: {
  outputDirectory?: string;
  descriptorPath?: string;
}): Promise<FixtureManifest>;
