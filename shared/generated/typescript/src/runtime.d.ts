export interface DecodeOptions {
  maxBytes?: number;
  maxDepth?: number;
}
export interface JsonOptions {
  emitDefaults?: boolean;
}
export interface MessageSchema<T> {
  readonly typeName: string;
  readonly fields: readonly unknown[];
}
export declare class ProtobufError extends Error {
  readonly code: string;
  constructor(message: string, code?: string);
}
export declare function defineEnum<T extends Record<string, number>>(typeName: string, values: T): Readonly<T>;
export declare function defineMessage<T>(typeName: string, fields: readonly unknown[]): MessageSchema<T>;
export declare function getMessageSchema<T = Record<string, unknown>>(typeName: string): MessageSchema<T>;
export declare function getEnumDescriptor(typeName: string): Readonly<{ typeName: string; values: Readonly<Record<string, number>> }>;
export declare function create<T>(schema: MessageSchema<T>, initializer?: Partial<T>): T;
export declare function toBinary<T>(schema: MessageSchema<T>, message: T): Uint8Array;
export declare function fromBinary<T>(schema: MessageSchema<T>, bytes: Uint8Array, options?: DecodeOptions): T;
export declare function toJson<T>(schema: MessageSchema<T>, message: T, options?: JsonOptions): Record<string, unknown>;
export declare function fromJson<T>(schema: MessageSchema<T>, json: unknown): T;
export declare function toJsonString<T>(schema: MessageSchema<T>, message: T, options?: JsonOptions): string;
export declare function fromJsonString<T>(schema: MessageSchema<T>, json: string): T;
export declare function clone<T>(schema: MessageSchema<T>, message: T): T;
export declare function equals<T>(schema: MessageSchema<T>, left: T, right: T): boolean;
