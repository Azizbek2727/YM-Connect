import type {
  CapabilityNegotiation,
  CapabilitySet,
  ClientEnvelope,
  ConnectorEnvelope,
  MessageSchema,
  ProtocolError,
  ProtocolSelection,
  ProtocolVersion,
  VersionRange,
} from "@ym-connect/protocol";

export declare const REPOSITORY_VERSION: "0.1.0";
export declare const PROTOCOL_VERSION: Readonly<ProtocolVersion>;
export declare const MIN_SUPPORTED_PROTOCOL_VERSION: Readonly<ProtocolVersion>;
export declare const MAX_SUPPORTED_PROTOCOL_VERSION: Readonly<ProtocolVersion>;
export declare const SUPPORTED_PROTOCOL_RANGE: Readonly<{
  minimum: Readonly<ProtocolVersion>;
  maximum: Readonly<ProtocolVersion>;
}>;
export declare function formatProtocolVersion(version: ProtocolVersion): string;
export declare function parseProtocolVersion(value: string): Readonly<ProtocolVersion>;
export declare function normalizeProtocolVersion(version: ProtocolVersion): Readonly<ProtocolVersion>;

export interface YmConnectErrorOptions extends ErrorOptions {
  domain?: number;
  code?: number;
  retryable?: boolean;
  retryAfterMs?: number;
  metadata?: Readonly<Record<string, string>>;
}
export declare class YmConnectError extends Error {
  readonly domain: number;
  readonly code: number;
  readonly retryable: boolean;
  readonly retryAfterMs: number;
  readonly metadata: Readonly<Record<string, string>>;
  constructor(message: string, options?: YmConnectErrorOptions);
}
export declare class ValidationError extends YmConnectError {}
export declare class CompatibilityError extends YmConnectError {}
export declare class CapabilityError extends YmConnectError {}
export declare class FramingError extends YmConnectError {}
export declare function toProtocolError(error: unknown): ProtocolError;
export declare function fromProtocolError(error: ProtocolError, options?: ErrorOptions): YmConnectError;
export declare function isProtocolError(value: unknown): value is ProtocolError;

export declare const DEFAULT_BASE_CAPABILITIES: readonly number[];
export declare function capabilityName(capability: number): string;
export declare function normalizeCapabilityList(
  values: readonly number[],
  options?: { allowUnknown?: boolean },
): readonly number[];
export declare function normalizeCapabilitySet(
  value: CapabilitySet,
  options?: { allowUnknown?: boolean },
): CapabilitySet;
export declare function hasCapability(capabilitySet: CapabilitySet, capability: number): boolean;
export declare function requireCapabilities(
  capabilitySet: CapabilitySet,
  requiredCapabilities: readonly number[],
): CapabilitySet;
export declare function negotiateCapabilities(
  local: CapabilitySet,
  remote: CapabilitySet,
  options?: { allowUnknown?: boolean },
): CapabilityNegotiation;

export declare function compareProtocolVersions(left: ProtocolVersion, right: ProtocolVersion): number;
export declare function normalizeVersionRange(value?: VersionRange): VersionRange;
export declare function versionInRange(version: ProtocolVersion, range: VersionRange): boolean;
export declare function intersectVersionRanges(left: VersionRange, right: VersionRange): VersionRange | undefined;
export declare function selectProtocolVersion(local: VersionRange, remote: VersionRange): ProtocolVersion | undefined;
export declare function assertProtocolCompatible(local: VersionRange, remote: VersionRange): ProtocolVersion;
export declare function negotiateProtocol(
  localRange: VersionRange,
  remoteRange: VersionRange,
  localCapabilities: CapabilitySet,
  remoteCapabilities: CapabilitySet,
): ProtocolSelection;

export interface CryptoRandomProvider {
  getRandomValues<T extends ArrayBufferView>(array: T): T;
}
export declare function generateOpaqueId(prefix: string, byteLength?: number, cryptoProvider?: CryptoRandomProvider): string;
export declare function generateMessageId(cryptoProvider?: CryptoRandomProvider): string;
export declare function generateCommandId(cryptoProvider?: CryptoRandomProvider): string;
export declare function generateSessionId(cryptoProvider?: CryptoRandomProvider): string;
export declare function generateInstanceId(cryptoProvider?: CryptoRandomProvider): string;
export declare function isOpaqueId(value: unknown, expectedPrefix?: string): value is string;
export declare function assertOpaqueId(value: unknown, expectedPrefix?: string): string;

export interface Clock {
  nowUnixMs(): bigint;
}
export declare const systemClock: Clock;
export interface MessageCodec<T> {
  encode(message: T): Uint8Array;
  decode(bytes: ArrayBuffer | ArrayBufferView, options?: unknown): T;
  toJson(message: T, options?: unknown): unknown;
  fromJson(json: unknown): T;
}
export declare function createCodec<T>(schema: MessageSchema<T>): MessageCodec<T>;
export interface Validator<T> {
  validate(value: T): T;
}
export declare function createValidator<T>(validate: (value: T) => T): Validator<T>;
export interface CompatibilityPolicy<TInput, TResult> {
  negotiate(input: TInput): TResult;
}
export declare function createCompatibilityPolicy<TInput, TResult>(
  negotiate: (input: TInput) => TResult,
): CompatibilityPolicy<TInput, TResult>;
export interface CapabilityProvider {
  getCapabilities(): CapabilitySet | Promise<CapabilitySet>;
}
export declare function createCapabilityProvider(
  getCapabilities: () => CapabilitySet | Promise<CapabilitySet>,
): CapabilityProvider;
export declare function assertClock(value: unknown): Clock;

export declare const DEFAULT_MAX_FRAME_SIZE: 1048576;
export declare function serializeBinary<T>(schema: MessageSchema<T>, message: T): Uint8Array;
export declare function deserializeBinary<T>(
  schema: MessageSchema<T>,
  bytes: ArrayBuffer | ArrayBufferView,
  options?: unknown,
): T;
export declare function serializeJson<T>(schema: MessageSchema<T>, message: T, options?: unknown): unknown;
export declare function deserializeJson<T>(schema: MessageSchema<T>, json: unknown): T;
export declare function serializeJsonString<T>(schema: MessageSchema<T>, message: T, options?: unknown): string;
export declare function deserializeJsonString<T>(schema: MessageSchema<T>, json: string): T;
export declare function encodeDelimited<T>(schema: MessageSchema<T>, message: T, maxFrameSize?: number): Uint8Array;
export declare function decodeDelimited<T>(
  schema: MessageSchema<T>,
  bytes: ArrayBuffer | ArrayBufferView,
  maxFrameSize?: number,
  options?: unknown,
): T;
export declare class DelimitedFrameDecoder {
  constructor(maxFrameSize?: number);
  get bufferedByteLength(): number;
  push(chunk: ArrayBuffer | ArrayBufferView): Uint8Array[];
  decode<T>(schema: MessageSchema<T>, chunk: ArrayBuffer | ArrayBufferView, options?: unknown): T[];
  reset(): void;
}

export declare const VALIDATION_LIMITS: Readonly<{
  identifier: number;
  displayName: number;
  provider: number;
  title: number;
  text: number;
  url: number;
  metadataEntries: number;
  binaryField: number;
  envelope: number;
}>;
export declare function validateIdentifier(
  value: string,
  fieldName?: string,
  options?: { allowEmpty?: boolean; maxLength?: number },
): string;
export declare function validateProtocolVersion(value: ProtocolVersion): Readonly<ProtocolVersion>;
export declare function validateMessageHeader<T>(value: T): T;
export declare function validatePlayerSnapshot<T>(value: T): T;
export declare function validateCommandRequest<T>(value: T): T;
export declare function validateEncryptedFrame<T>(value: T): T;
export declare function validateConnectorEnvelope(value: ConnectorEnvelope, schema?: MessageSchema<ConnectorEnvelope>): ConnectorEnvelope;
export declare function validateClientEnvelope(value: ClientEnvelope, schema?: MessageSchema<ClientEnvelope>): ClientEnvelope;
export declare function validateSerializedSize<T>(schema: MessageSchema<T>, message: T, maximum?: number): number;
export declare function validateProtocolError<T extends ProtocolError>(value: T): T;
export declare function validateCapabilitySet(value: CapabilitySet): CapabilitySet;
