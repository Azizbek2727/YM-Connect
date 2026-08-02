import {
  fromBinary,
  fromJson,
  fromJsonString,
  toBinary,
  toJson,
  toJsonString,
} from "@ym-connect/protocol";
import { FramingError } from "./errors.js";

export const DEFAULT_MAX_FRAME_SIZE = 1_048_576;

export function serializeBinary(schema, message) {
  return toBinary(schema, message);
}

export function deserializeBinary(schema, bytes, options) {
  return fromBinary(schema, normalizeBytes(bytes), options);
}

export function serializeJson(schema, message, options) {
  return toJson(schema, message, options);
}

export function deserializeJson(schema, json) {
  return fromJson(schema, json);
}

export function serializeJsonString(schema, message, options) {
  return toJsonString(schema, message, options);
}

export function deserializeJsonString(schema, json) {
  return fromJsonString(schema, json);
}

export function encodeDelimited(schema, message, maxFrameSize = DEFAULT_MAX_FRAME_SIZE) {
  validateMaxFrameSize(maxFrameSize);
  const payload = serializeBinary(schema, message);
  if (payload.byteLength > maxFrameSize) {
    throw new FramingError(`encoded frame exceeds ${maxFrameSize} bytes`);
  }
  const framed = new Uint8Array(4 + payload.byteLength);
  new DataView(framed.buffer, framed.byteOffset, 4).setUint32(0, payload.byteLength, false);
  framed.set(payload, 4);
  return framed;
}

export function decodeDelimited(schema, bytes, maxFrameSize = DEFAULT_MAX_FRAME_SIZE, options) {
  validateMaxFrameSize(maxFrameSize);
  const framed = normalizeBytes(bytes);
  if (framed.byteLength < 4) throw new FramingError("frame header is incomplete");
  const length = new DataView(framed.buffer, framed.byteOffset, 4).getUint32(0, false);
  if (length > maxFrameSize)
    throw new FramingError(`frame declares ${length} bytes, exceeding the limit`);
  if (framed.byteLength !== length + 4) {
    throw new FramingError(
      `frame length mismatch: declared ${length}, received ${framed.byteLength - 4}`,
    );
  }
  return deserializeBinary(schema, framed.subarray(4), options);
}

export class DelimitedFrameDecoder {
  #buffer = new Uint8Array(0);
  #maxFrameSize;

  constructor(maxFrameSize = DEFAULT_MAX_FRAME_SIZE) {
    validateMaxFrameSize(maxFrameSize);
    this.#maxFrameSize = maxFrameSize;
  }

  get bufferedByteLength() {
    return this.#buffer.byteLength;
  }

  push(chunk) {
    const incoming = normalizeBytes(chunk);
    if (incoming.byteLength === 0) return [];
    const combined = new Uint8Array(this.#buffer.byteLength + incoming.byteLength);
    combined.set(this.#buffer);
    combined.set(incoming, this.#buffer.byteLength);
    this.#buffer = combined;
    const frames = [];
    let offset = 0;
    while (this.#buffer.byteLength - offset >= 4) {
      const view = new DataView(this.#buffer.buffer, this.#buffer.byteOffset + offset, 4);
      const length = view.getUint32(0, false);
      if (length > this.#maxFrameSize) {
        this.reset();
        throw new FramingError(`frame declares ${length} bytes, exceeding the limit`);
      }
      const total = 4 + length;
      if (this.#buffer.byteLength - offset < total) break;
      frames.push(this.#buffer.slice(offset + 4, offset + total));
      offset += total;
    }
    if (offset > 0) this.#buffer = this.#buffer.slice(offset);
    if (this.#buffer.byteLength > this.#maxFrameSize + 4) {
      this.reset();
      throw new FramingError("buffered frame exceeds the configured limit");
    }
    return frames;
  }

  decode(schema, chunk, options) {
    return this.push(chunk).map((frame) => deserializeBinary(schema, frame, options));
  }

  reset() {
    this.#buffer = new Uint8Array(0);
  }
}

function normalizeBytes(bytes) {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  if (ArrayBuffer.isView(bytes)) {
    return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  throw new TypeError("binary input must be an ArrayBuffer or ArrayBuffer view");
}

function validateMaxFrameSize(value) {
  if (!Number.isSafeInteger(value) || value < 1 || value > 0xffff_ffff) {
    throw new RangeError("maxFrameSize must be a positive unsigned 32-bit integer");
  }
}
