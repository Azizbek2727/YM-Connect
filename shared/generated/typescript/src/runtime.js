const messageRegistry = new Map();
const enumRegistry = new Map();
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder("utf-8", { fatal: true });
const PACKABLE = new Set([
  "double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64",
  "fixed32", "fixed64", "sfixed32", "sfixed64", "bool",
]);
const INTEGER_RANGES = Object.freeze({
  int32: [-0x8000_0000, 0x7fff_ffff],
  uint32: [0, 0xffff_ffff],
  sint32: [-0x8000_0000, 0x7fff_ffff],
  fixed32: [0, 0xffff_ffff],
  sfixed32: [-0x8000_0000, 0x7fff_ffff],
});
const BIGINT_RANGES = Object.freeze({
  int64: [-0x8000_0000_0000_0000n, 0x7fff_ffff_ffff_ffffn],
  uint64: [0n, 0xffff_ffff_ffff_ffffn],
  sint64: [-0x8000_0000_0000_0000n, 0x7fff_ffff_ffff_ffffn],
  fixed64: [0n, 0xffff_ffff_ffff_ffffn],
  sfixed64: [-0x8000_0000_0000_0000n, 0x7fff_ffff_ffff_ffffn],
});

export class ProtobufError extends Error {
  constructor(message, code = "PROTOBUF_ERROR") {
    super(message);
    this.name = "ProtobufError";
    this.code = code;
  }
}

export function defineEnum(typeName, values) {
  if (enumRegistry.has(typeName)) throw new ProtobufError(`duplicate enum ${typeName}`, "DUPLICATE_TYPE");
  const frozen = Object.freeze({ ...values });
  const byNumber = new Map();
  for (const [name, number] of Object.entries(frozen)) {
    if (!Number.isInteger(number)) throw new ProtobufError(`enum ${typeName}.${name} is not an integer`);
    if (!byNumber.has(number)) byNumber.set(number, name);
  }
  enumRegistry.set(typeName, { typeName, values: frozen, byNumber });
  return frozen;
}

export function defineMessage(typeName, fields) {
  if (messageRegistry.has(typeName)) throw new ProtobufError(`duplicate message ${typeName}`, "DUPLICATE_TYPE");
  const numbers = new Set();
  const names = new Set();
  const normalized = fields.map((field) => {
    if (!Number.isInteger(field.no) || field.no <= 0 || field.no >= 536870912) {
      throw new ProtobufError(`invalid field number ${field.no} in ${typeName}`);
    }
    if (numbers.has(field.no) || names.has(field.name)) throw new ProtobufError(`duplicate field in ${typeName}`);
    numbers.add(field.no);
    names.add(field.name);
    return Object.freeze({ ...field });
  });
  const schema = Object.freeze({
    typeName,
    fields: Object.freeze(normalized),
    fieldsByNumber: new Map(normalized.map((field) => [field.no, field])),
  });
  messageRegistry.set(typeName, schema);
  return schema;
}

export function getMessageSchema(typeName) {
  const schema = messageRegistry.get(typeName);
  if (!schema) throw new ProtobufError(`unknown message type ${typeName}`, "UNKNOWN_TYPE");
  return schema;
}

export function getEnumDescriptor(typeName) {
  const descriptor = enumRegistry.get(typeName);
  if (!descriptor) throw new ProtobufError(`unknown enum type ${typeName}`, "UNKNOWN_TYPE");
  return descriptor;
}

function scalarDefault(type) {
  if (["int64", "uint64", "sint64", "fixed64", "sfixed64"].includes(type)) return 0n;
  if (type === "bool") return false;
  if (type === "string") return "";
  if (type === "bytes") return new Uint8Array();
  return 0;
}

function fieldDefault(field) {
  if (field.repeated) return [];
  if (field.kind === "map") return {};
  if (field.kind === "scalar") return scalarDefault(field.scalar);
  if (field.kind === "enum") return 0;
  return undefined;
}

function cloneValue(field, value) {
  if (value === undefined) return undefined;
  if (field.repeated) return value.map((entry) => cloneSingle(field, entry));
  if (field.kind === "map") {
    const result = {};
    for (const [key, entry] of Object.entries(value)) result[key] = cloneMapValue(field.value, entry);
    return result;
  }
  return cloneSingle(field, value);
}

function cloneSingle(field, value) {
  if (field.kind === "message") return create(getMessageSchema(field.typeName), value);
  if (field.kind === "scalar" && field.scalar === "bytes") return Uint8Array.from(value);
  return value;
}

function cloneMapValue(descriptor, value) {
  if (descriptor.kind === "message") return create(getMessageSchema(descriptor.typeName), value);
  if (descriptor.kind === "scalar" && descriptor.scalar === "bytes") return Uint8Array.from(value);
  return value;
}

export function create(schema, initializer = {}) {
  const message = {};
  const oneofs = new Set();
  for (const field of schema.fields) {
    if (field.oneof) {
      if (!oneofs.has(field.oneof)) {
        message[field.oneof] = { case: undefined };
        oneofs.add(field.oneof);
      }
    } else {
      message[field.name] = fieldDefault(field);
    }
  }
  for (const [key, value] of Object.entries(initializer)) {
    const field = schema.fields.find((candidate) => candidate.name === key);
    if (field) {
      if (field.oneof) throw new ProtobufError(`initialize oneof ${field.oneof} instead of ${field.name}`);
      message[key] = cloneValue(field, value);
      continue;
    }
    const oneofFields = schema.fields.filter((candidate) => candidate.oneof === key);
    if (oneofFields.length > 0) {
      if (value === undefined || value.case === undefined) {
        message[key] = { case: undefined };
      } else {
        const selected = oneofFields.find((candidate) => candidate.name === value.case);
        if (!selected) throw new ProtobufError(`invalid oneof case ${String(value.case)} for ${key}`);
        message[key] = { case: selected.name, value: cloneSingle(selected, value.value) };
      }
      continue;
    }
    throw new ProtobufError(`unknown field ${key} for ${schema.typeName}`);
  }
  return message;
}

class Writer {
  constructor() { this.bytes = []; }
  tag(no, wire) { this.varint(BigInt((no << 3) | wire)); }
  varint(value) {
    let current = BigInt.asUintN(64, BigInt(value));
    while (current >= 0x80n) {
      this.bytes.push(Number((current & 0x7fn) | 0x80n));
      current >>= 7n;
    }
    this.bytes.push(Number(current));
  }
  fixed32(value) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setUint32(0, Number(value) >>> 0, true);
    this.raw(bytes);
  }
  fixed64(value) {
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setBigUint64(0, BigInt.asUintN(64, BigInt(value)), true);
    this.raw(bytes);
  }
  float(value) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setFloat32(0, value, true);
    this.raw(bytes);
  }
  double(value) {
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setFloat64(0, value, true);
    this.raw(bytes);
  }
  raw(bytes) { for (const byte of bytes) this.bytes.push(byte); }
  lengthDelimited(bytes) { this.varint(BigInt(bytes.length)); this.raw(bytes); }
  finish() { return Uint8Array.from(this.bytes); }
}

class Reader {
  constructor(bytes, options, depth = 0) {
    if (!(bytes instanceof Uint8Array)) throw new ProtobufError("binary input must be Uint8Array", "INVALID_INPUT");
    if (bytes.length > options.maxBytes) throw new ProtobufError(`message exceeds ${options.maxBytes} bytes`, "MESSAGE_TOO_LARGE");
    if (depth > options.maxDepth) throw new ProtobufError(`message exceeds depth ${options.maxDepth}`, "DEPTH_LIMIT");
    this.bytes = bytes;
    this.offset = 0;
    this.options = options;
    this.depth = depth;
  }
  eof() { return this.offset >= this.bytes.length; }
  ensure(count) {
    if (!Number.isSafeInteger(count) || count < 0 || this.offset + count > this.bytes.length) {
      throw new ProtobufError("truncated protobuf input", "TRUNCATED_INPUT");
    }
  }
  varint() {
    let value = 0n;
    let shift = 0n;
    for (let count = 0; count < 10; count += 1) {
      this.ensure(1);
      const byte = this.bytes[this.offset++];
      if (count === 9 && byte > 1) throw new ProtobufError("varint exceeds 64-bit range", "INVALID_VARINT");
      value |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return value;
      shift += 7n;
    }
    throw new ProtobufError("varint exceeds 10 bytes", "INVALID_VARINT");
  }
  fixed32() { this.ensure(4); const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.offset, 4).getUint32(0, true); this.offset += 4; return value; }
  fixed64() { this.ensure(8); const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.offset, 8).getBigUint64(0, true); this.offset += 8; return value; }
  float() { this.ensure(4); const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.offset, 4).getFloat32(0, true); this.offset += 4; return value; }
  double() { this.ensure(8); const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.offset, 8).getFloat64(0, true); this.offset += 8; return value; }
  bytesValue() {
    const length = Number(this.varint());
    if (!Number.isSafeInteger(length)) throw new ProtobufError("length exceeds safe integer range", "INVALID_LENGTH");
    this.ensure(length);
    const value = this.bytes.subarray(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }
  child(bytes) { return new Reader(bytes, this.options, this.depth + 1); }
  skip(wire) {
    if (wire === 0) this.varint();
    else if (wire === 1) { this.ensure(8); this.offset += 8; }
    else if (wire === 2) this.bytesValue();
    else if (wire === 5) { this.ensure(4); this.offset += 4; }
    else throw new ProtobufError(`unsupported wire type ${wire}`, "INVALID_WIRE_TYPE");
  }
}

function zigZagEncode(value, bits) {
  const input = BigInt(value);
  return BigInt.asUintN(bits, (input << 1n) ^ (input >> BigInt(bits - 1)));
}
function zigZagDecode(value) { return (value >> 1n) ^ -(value & 1n); }
function wireTypeForScalar(type) {
  if (["fixed64", "sfixed64", "double"].includes(type)) return 1;
  if (["string", "bytes"].includes(type)) return 2;
  if (["fixed32", "sfixed32", "float"].includes(type)) return 5;
  return 0;
}
function isDefaultScalar(type, value) {
  if (type === "bytes") return value.length === 0;
  return value === scalarDefault(type);
}

function writeScalar(writer, type, value) {
  switch (type) {
    case "double": writer.double(value); break;
    case "float": writer.float(value); break;
    case "int32": writer.varint(BigInt.asUintN(64, BigInt(value | 0))); break;
    case "int64": writer.varint(BigInt.asUintN(64, BigInt(value))); break;
    case "uint32": writer.varint(BigInt(Number(value) >>> 0)); break;
    case "uint64": writer.varint(BigInt.asUintN(64, BigInt(value))); break;
    case "sint32": writer.varint(zigZagEncode(BigInt(value | 0), 32)); break;
    case "sint64": writer.varint(zigZagEncode(BigInt(value), 64)); break;
    case "fixed32": writer.fixed32(value); break;
    case "fixed64": writer.fixed64(value); break;
    case "sfixed32": writer.fixed32(value); break;
    case "sfixed64": writer.fixed64(value); break;
    case "bool": writer.varint(value ? 1n : 0n); break;
    case "string": writer.lengthDelimited(textEncoder.encode(value)); break;
    case "bytes": writer.lengthDelimited(value); break;
    default: throw new ProtobufError(`unsupported scalar ${type}`);
  }
}

function readScalar(reader, type, wire) {
  const expected = wireTypeForScalar(type);
  if (wire !== expected) throw new ProtobufError(`wire type ${wire} does not match ${type}`, "WIRE_TYPE_MISMATCH");
  switch (type) {
    case "double": return reader.double();
    case "float": return reader.float();
    case "int32": return Number(BigInt.asIntN(32, reader.varint()));
    case "int64": return BigInt.asIntN(64, reader.varint());
    case "uint32": return Number(BigInt.asUintN(32, reader.varint()));
    case "uint64": return BigInt.asUintN(64, reader.varint());
    case "sint32": return Number(BigInt.asIntN(32, zigZagDecode(reader.varint())));
    case "sint64": return BigInt.asIntN(64, zigZagDecode(reader.varint()));
    case "fixed32": return reader.fixed32();
    case "fixed64": return reader.fixed64();
    case "sfixed32": return reader.fixed32() | 0;
    case "sfixed64": return BigInt.asIntN(64, reader.fixed64());
    case "bool": return reader.varint() !== 0n;
    case "string": return textDecoder.decode(reader.bytesValue());
    case "bytes": return Uint8Array.from(reader.bytesValue());
    default: throw new ProtobufError(`unsupported scalar ${type}`);
  }
}

function writeSingle(writer, field, value) {
  if (field.kind === "scalar") {
    writer.tag(field.no, wireTypeForScalar(field.scalar));
    writeScalar(writer, field.scalar, value);
  } else if (field.kind === "enum") {
    writer.tag(field.no, 0); writer.varint(BigInt(value));
  } else if (field.kind === "message") {
    writer.tag(field.no, 2); writer.lengthDelimited(toBinary(getMessageSchema(field.typeName), value));
  } else {
    throw new ProtobufError(`cannot write field kind ${field.kind}`);
  }
}

function writeMap(writer, field, map) {
  for (const key of Object.keys(map).sort()) {
    const entry = new Writer();
    entry.tag(1, wireTypeForScalar(field.keyScalar));
    writeScalar(entry, field.keyScalar, key);
    const value = map[key];
    if (field.value.kind === "scalar") {
      entry.tag(2, wireTypeForScalar(field.value.scalar)); writeScalar(entry, field.value.scalar, value);
    } else if (field.value.kind === "enum") {
      entry.tag(2, 0); entry.varint(BigInt(value));
    } else {
      entry.tag(2, 2); entry.lengthDelimited(toBinary(getMessageSchema(field.value.typeName), value));
    }
    writer.tag(field.no, 2); writer.lengthDelimited(entry.finish());
  }
}

export function toBinary(schema, message) {
  const writer = new Writer();
  for (const field of schema.fields) {
    let value;
    if (field.oneof) {
      const selected = message[field.oneof];
      if (!selected || selected.case !== field.name) continue;
      value = selected.value;
    } else value = message[field.name];
    if (field.repeated) {
      if (!Array.isArray(value) || value.length === 0) continue;
      if ((field.kind === "scalar" && PACKABLE.has(field.scalar)) || field.kind === "enum") {
        const packed = new Writer();
        for (const entry of value) {
          if (field.kind === "enum") packed.varint(BigInt(entry)); else writeScalar(packed, field.scalar, entry);
        }
        writer.tag(field.no, 2); writer.lengthDelimited(packed.finish());
      } else {
        for (const entry of value) writeSingle(writer, field, entry);
      }
      continue;
    }
    if (field.kind === "map") { if (value && Object.keys(value).length > 0) writeMap(writer, field, value); continue; }
    if (field.kind === "message") { if (value !== undefined) writeSingle(writer, field, value); continue; }
    if (field.kind === "enum") { if (value !== 0) writeSingle(writer, field, value); continue; }
    if (!isDefaultScalar(field.scalar, value)) writeSingle(writer, field, value);
  }
  return writer.finish();
}

function readSingle(reader, field, wire) {
  if (field.kind === "scalar") return readScalar(reader, field.scalar, wire);
  if (field.kind === "enum") {
    if (wire !== 0) throw new ProtobufError("enum requires varint wire type", "WIRE_TYPE_MISMATCH");
    return Number(BigInt.asIntN(32, reader.varint()));
  }
  if (field.kind === "message") {
    if (wire !== 2) throw new ProtobufError("message requires length-delimited wire type", "WIRE_TYPE_MISMATCH");
    return readMessage(reader.child(reader.bytesValue()), getMessageSchema(field.typeName));
  }
  throw new ProtobufError(`cannot read field kind ${field.kind}`);
}

function readMap(reader, field, wire) {
  if (wire !== 2) throw new ProtobufError("map requires length-delimited wire type", "WIRE_TYPE_MISMATCH");
  const entryReader = reader.child(reader.bytesValue());
  let key = scalarDefault(field.keyScalar);
  let value = field.value.kind === "scalar" ? scalarDefault(field.value.scalar) : field.value.kind === "enum" ? 0 : undefined;
  while (!entryReader.eof()) {
    const rawTag = entryReader.varint();
    if (rawTag > 0xffff_ffffn) throw new ProtobufError("field tag exceeds 32-bit range", "INVALID_TAG");
    const tag = Number(rawTag);
    const no = tag >>> 3; const entryWire = tag & 7;
    if (no === 1) key = readScalar(entryReader, field.keyScalar, entryWire);
    else if (no === 2) value = readSingle(entryReader, { no: 2, ...field.value }, entryWire);
    else entryReader.skip(entryWire);
  }
  return [String(key), value];
}

function readPacked(reader, field) {
  const packed = reader.child(reader.bytesValue());
  const result = [];
  while (!packed.eof()) {
    if (field.kind === "enum") result.push(Number(BigInt.asIntN(32, packed.varint())));
    else result.push(readScalar(packed, field.scalar, wireTypeForScalar(field.scalar)));
  }
  return result;
}

function readMessage(reader, schema) {
  const message = create(schema);
  while (!reader.eof()) {
    const rawTag = reader.varint();
    if (rawTag === 0n || rawTag > 0xffff_ffffn) {
      throw new ProtobufError("field tag is outside the valid 32-bit range", "INVALID_TAG");
    }
    const tag = Number(rawTag);
    const no = tag >>> 3; const wire = tag & 7;
    const field = schema.fieldsByNumber.get(no);
    if (!field) { reader.skip(wire); continue; }
    if (field.kind === "map") {
      const [key, value] = readMap(reader, field, wire);
      message[field.name][key] = value;
      continue;
    }
    if (field.repeated) {
      const packed = wire === 2 && ((field.kind === "scalar" && PACKABLE.has(field.scalar)) || field.kind === "enum");
      if (packed) message[field.name].push(...readPacked(reader, field));
      else message[field.name].push(readSingle(reader, field, wire));
      continue;
    }
    const value = readSingle(reader, field, wire);
    if (field.oneof) message[field.oneof] = { case: field.name, value };
    else message[field.name] = value;
  }
  return message;
}

export function fromBinary(schema, bytes, options = {}) {
  const normalized = { maxBytes: options.maxBytes ?? 1048576, maxDepth: options.maxDepth ?? 64 };
  if (!Number.isSafeInteger(normalized.maxBytes) || normalized.maxBytes < 1) {
    throw new RangeError("maxBytes must be a positive safe integer");
  }
  if (!Number.isSafeInteger(normalized.maxDepth) || normalized.maxDepth < 0) {
    throw new RangeError("maxDepth must be a non-negative safe integer");
  }
  return readMessage(new Reader(bytes, normalized), schema);
}

function bytesToBase64(bytes) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
function base64ToBytes(value) {
  if (value.length % 4 !== 0 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) {
    throw new ProtobufError("bytes JSON must contain canonical base64", "INVALID_JSON");
  }
  try {
    const binary = atob(value);
    return Uint8Array.from(binary, (char) => char.charCodeAt(0));
  } catch {
    throw new ProtobufError("bytes JSON must contain canonical base64", "INVALID_JSON");
  }
}
function enumToJson(typeName, value) { return getEnumDescriptor(typeName).byNumber.get(value) ?? value; }
function enumFromJson(typeName, value) {
  if (typeof value === "number" && Number.isInteger(value)) {
    if (value < -0x8000_0000 || value > 0x7fff_ffff) {
      throw new ProtobufError(`enum JSON is out of range for ${typeName}`, "INVALID_JSON");
    }
    return value;
  }
  if (typeof value !== "string") throw new ProtobufError(`invalid enum JSON for ${typeName}`, "INVALID_JSON");
  const descriptor = getEnumDescriptor(typeName);
  const result = descriptor.values[value];
  if (result === undefined) throw new ProtobufError(`unknown enum name ${value} for ${typeName}`, "INVALID_JSON");
  return result;
}
function scalarToJson(type, value) {
  if (["int64", "uint64", "sint64", "fixed64", "sfixed64"].includes(type)) return value.toString();
  if (type === "bytes") return bytesToBase64(value);
  if (type === "double" || type === "float") {
    if (Number.isNaN(value)) return "NaN";
    if (value === Infinity) return "Infinity";
    if (value === -Infinity) return "-Infinity";
  }
  return value;
}
function scalarFromJson(type, value) {
  if (Object.hasOwn(BIGINT_RANGES, type)) {
    if (typeof value !== "string" && typeof value !== "number") {
      throw new ProtobufError(`invalid ${type} JSON`, "INVALID_JSON");
    }
    if (typeof value === "number" && (!Number.isSafeInteger(value) || !Number.isInteger(value))) {
      throw new ProtobufError(`${type} JSON number must be a safe integer`, "INVALID_JSON");
    }
    let parsed;
    try { parsed = BigInt(value); }
    catch { throw new ProtobufError(`invalid ${type} JSON`, "INVALID_JSON"); }
    const [minimum, maximum] = BIGINT_RANGES[type];
    if (parsed < minimum || parsed > maximum) throw new ProtobufError(`${type} JSON is out of range`, "INVALID_JSON");
    return parsed;
  }
  if (type === "bytes") {
    if (typeof value !== "string") throw new ProtobufError("bytes JSON must be base64 string", "INVALID_JSON");
    return base64ToBytes(value);
  }
  if (type === "bool") {
    if (typeof value !== "boolean") throw new ProtobufError("bool JSON must be boolean", "INVALID_JSON");
    return value;
  }
  if (type === "string") {
    if (typeof value !== "string") throw new ProtobufError("string JSON must be string", "INVALID_JSON");
    return value;
  }
  if ((type === "double" || type === "float") && typeof value === "string") {
    if (value === "NaN") return Number.NaN;
    if (value === "Infinity") return Infinity;
    if (value === "-Infinity") return -Infinity;
  }
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new ProtobufError(`${type} JSON must be a finite number`, "INVALID_JSON");
  }
  if (Object.hasOwn(INTEGER_RANGES, type)) {
    if (!Number.isInteger(value)) throw new ProtobufError(`${type} JSON must be an integer`, "INVALID_JSON");
    const [minimum, maximum] = INTEGER_RANGES[type];
    if (value < minimum || value > maximum) throw new ProtobufError(`${type} JSON is out of range`, "INVALID_JSON");
  }
  if (type === "float" && Math.abs(value) > 3.4028234663852886e38) {
    throw new ProtobufError("float JSON is out of range", "INVALID_JSON");
  }
  return value;
}
function singleToJson(field, value) {
  if (field.kind === "scalar") return scalarToJson(field.scalar, value);
  if (field.kind === "enum") return enumToJson(field.typeName, value);
  return toJson(getMessageSchema(field.typeName), value);
}
function singleFromJson(field, value) {
  if (field.kind === "scalar") return scalarFromJson(field.scalar, value);
  if (field.kind === "enum") return enumFromJson(field.typeName, value);
  return fromJson(getMessageSchema(field.typeName), value);
}

export function toJson(schema, message, options = {}) {
  const emitDefaults = options.emitDefaults ?? false;
  const result = {};
  for (const field of schema.fields) {
    let value;
    if (field.oneof) {
      const selected = message[field.oneof];
      if (!selected || selected.case !== field.name) continue;
      value = selected.value;
    } else value = message[field.name];
    if (field.repeated) {
      if (emitDefaults || value.length > 0) result[field.protoName] = value.map((entry) => singleToJson(field, entry));
    } else if (field.kind === "map") {
      if (emitDefaults || Object.keys(value).length > 0) {
        const map = {};
        for (const key of Object.keys(value).sort()) map[key] = singleToJson({ no: 2, ...field.value }, value[key]);
        result[field.protoName] = map;
      }
    } else if (field.kind === "message") {
      if (value !== undefined) result[field.protoName] = singleToJson(field, value);
    } else if (field.kind === "enum") {
      if (emitDefaults || value !== 0) result[field.protoName] = singleToJson(field, value);
    } else if (emitDefaults || !isDefaultScalar(field.scalar, value)) {
      result[field.protoName] = singleToJson(field, value);
    }
  }
  return result;
}

export function fromJson(schema, json) {
  if (json === null || typeof json !== "object" || Array.isArray(json)) {
    throw new ProtobufError(`JSON for ${schema.typeName} must be an object`, "INVALID_JSON");
  }
  const message = create(schema);
  const selectedOneofs = new Set();
  for (const [key, raw] of Object.entries(json)) {
    const field = schema.fields.find((candidate) => candidate.protoName === key || candidate.name === key);
    if (!field) throw new ProtobufError(`unknown JSON field ${key} for ${schema.typeName}`, "INVALID_JSON");
    if (raw === null) throw new ProtobufError(`${key} must not be null`, "INVALID_JSON");
    if (field.oneof) {
      if (selectedOneofs.has(field.oneof)) {
        throw new ProtobufError(`multiple JSON fields select oneof ${field.oneof}`, "INVALID_JSON");
      }
      selectedOneofs.add(field.oneof);
    }
    if (field.repeated) {
      if (!Array.isArray(raw)) throw new ProtobufError(`${key} must be an array`, "INVALID_JSON");
      message[field.name] = raw.map((entry) => singleFromJson(field, entry));
    } else if (field.kind === "map") {
      if (typeof raw !== "object" || Array.isArray(raw)) throw new ProtobufError(`${key} must be an object`, "INVALID_JSON");
      const map = {};
      for (const [mapKey, mapValue] of Object.entries(raw)) {
        map[mapKey] = singleFromJson({ no: 2, ...field.value }, mapValue);
      }
      message[field.name] = map;
    } else {
      const value = singleFromJson(field, raw);
      if (field.oneof) message[field.oneof] = { case: field.name, value };
      else message[field.name] = value;
    }
  }
  return message;
}
export function toJsonString(schema, message, options = {}) {
  return JSON.stringify(toJson(schema, message, options));
}
export function fromJsonString(schema, json) {
  try { return fromJson(schema, JSON.parse(json)); }
  catch (error) {
    if (error instanceof ProtobufError) throw error;
    throw new ProtobufError(error instanceof Error ? error.message : String(error), "INVALID_JSON");
  }
}
export function clone(schema, message) { return create(schema, message); }
export function equals(schema, left, right) {
  if (left === right) return true;
  return toJsonString(schema, left, { emitDefaults: true }) === toJsonString(schema, right, { emitDefaults: true });
}
