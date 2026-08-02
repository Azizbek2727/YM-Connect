#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { isAbsolute, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import process from "node:process";

const SCALARS = new Set([
  "double",
  "float",
  "int32",
  "int64",
  "uint32",
  "uint64",
  "sint32",
  "sint64",
  "fixed32",
  "fixed64",
  "sfixed32",
  "sfixed64",
  "bool",
  "string",
  "bytes",
]);

function stripComments(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/\/\/[^\n\r]*/g, " ");
}

function tokenize(source) {
  const input = stripComments(source);
  const tokens = [];
  let index = 0;
  while (index < input.length) {
    const char = input[index];
    if (/\s/.test(char)) {
      index += 1;
      continue;
    }
    if (char === '"') {
      let value = "";
      index += 1;
      while (index < input.length && input[index] !== '"') {
        if (input[index] === "\\") {
          index += 1;
          if (index >= input.length) throw new Error("unterminated string escape");
          const escaped = input[index];
          const mapping = { n: "\n", r: "\r", t: "\t", '"': '"', "\\": "\\" };
          value += mapping[escaped] ?? escaped;
          index += 1;
        } else {
          value += input[index++];
        }
      }
      if (input[index] !== '"') throw new Error("unterminated string literal");
      index += 1;
      tokens.push({ type: "string", value });
      continue;
    }
    if (/[A-Za-z_]/.test(char)) {
      let end = index + 1;
      while (end < input.length && /[A-Za-z0-9_]/.test(input[end])) end += 1;
      tokens.push({ type: "identifier", value: input.slice(index, end) });
      index = end;
      continue;
    }
    if (/[0-9-]/.test(char)) {
      let end = index + 1;
      while (end < input.length && /[0-9]/.test(input[end])) end += 1;
      tokens.push({ type: "number", value: input.slice(index, end) });
      index = end;
      continue;
    }
    if ("{}[]=;,.<>".includes(char)) {
      tokens.push({ type: "symbol", value: char });
      index += 1;
      continue;
    }
    throw new Error(`unsupported token ${JSON.stringify(char)} at offset ${index}`);
  }
  return tokens;
}

class Parser {
  constructor(tokens, fileName) {
    this.tokens = tokens;
    this.fileName = fileName;
    this.index = 0;
  }
  peek(value) {
    const token = this.tokens[this.index];
    return token !== undefined && (value === undefined || token.value === value);
  }
  take(value) {
    const token = this.tokens[this.index];
    if (token === undefined) throw new Error(`${this.fileName}: unexpected end of file`);
    if (value !== undefined && token.value !== value)
      throw new Error(`${this.fileName}: expected ${value}, received ${token.value}`);
    this.index += 1;
    return token;
  }
  takeIdentifier() {
    const token = this.take();
    if (token.type !== "identifier")
      throw new Error(`${this.fileName}: expected identifier, received ${token.value}`);
    return token.value;
  }
  takeNumber() {
    const token = this.take();
    if (token.type !== "number")
      throw new Error(`${this.fileName}: expected number, received ${token.value}`);
    return Number.parseInt(token.value, 10);
  }
  parseQualifiedName() {
    let result = "";
    if (this.peek(".")) result += this.take(".").value;
    result += this.takeIdentifier();
    while (this.peek(".")) {
      this.take(".");
      result += `.${this.takeIdentifier()}`;
    }
    return result;
  }
  skipStatement() {
    let depth = 0;
    while (this.index < this.tokens.length) {
      const token = this.take().value;
      if (token === "{" || token === "[") depth += 1;
      if (token === "}" || token === "]") depth -= 1;
      if (token === ";" && depth === 0) return;
    }
  }
  parseEnum() {
    this.take("enum");
    const name = this.takeIdentifier();
    this.take("{");
    const values = [];
    while (!this.peek("}")) {
      if (this.peek("option") || this.peek("reserved")) {
        this.skipStatement();
        continue;
      }
      const valueName = this.takeIdentifier();
      this.take("=");
      const number = this.takeNumber();
      if (this.peek("[")) {
        let depth = 0;
        do {
          const token = this.take().value;
          if (token === "[") depth += 1;
          if (token === "]") depth -= 1;
        } while (depth > 0);
      }
      this.take(";");
      values.push({ name: valueName, number });
    }
    this.take("}");
    return { name, values };
  }
  parseField(oneof = null) {
    let label = "optional";
    if (this.peek("repeated") || this.peek("optional") || this.peek("required"))
      label = this.take().value;
    let type;
    let map = null;
    if (this.peek("map")) {
      this.take("map");
      this.take("<");
      const keyType = this.parseQualifiedName();
      this.take(",");
      const valueType = this.parseQualifiedName();
      this.take(">");
      type = "map";
      map = { keyType, valueType };
      label = "repeated";
    } else {
      type = this.parseQualifiedName();
    }
    const name = this.takeIdentifier();
    this.take("=");
    const number = this.takeNumber();
    if (this.peek("[")) {
      let depth = 0;
      do {
        const token = this.take().value;
        if (token === "[") depth += 1;
        if (token === "]") depth -= 1;
      } while (depth > 0);
    }
    this.take(";");
    return { label, type, name, number, oneof, map };
  }
  parseMessage() {
    this.take("message");
    const name = this.takeIdentifier();
    this.take("{");
    const fields = [];
    const oneofs = [];
    while (!this.peek("}")) {
      if (this.peek("oneof")) {
        this.take("oneof");
        const oneofName = this.takeIdentifier();
        oneofs.push(oneofName);
        this.take("{");
        while (!this.peek("}")) fields.push(this.parseField(oneofName));
        this.take("}");
        continue;
      }
      if (
        this.peek("reserved") ||
        this.peek("option") ||
        this.peek("extensions") ||
        this.peek("extend")
      ) {
        this.skipStatement();
        continue;
      }
      if (this.peek("message") || this.peek("enum"))
        throw new Error(`${this.fileName}: nested declarations are unsupported`);
      fields.push(this.parseField());
    }
    this.take("}");
    return { name, fields, oneofs };
  }
  parseFile() {
    const file = {
      name: this.fileName,
      packageName: "",
      imports: [],
      options: {},
      enums: [],
      messages: [],
    };
    while (this.index < this.tokens.length) {
      if (this.peek("syntax")) this.skipStatement();
      else if (this.peek("package")) {
        this.take("package");
        file.packageName = this.parseQualifiedName();
        this.take(";");
      } else if (this.peek("import")) {
        this.take("import");
        if (this.peek("public") || this.peek("weak")) this.take();
        const token = this.take();
        if (token.type !== "string")
          throw new Error(`${this.fileName}: import path must be a string`);
        file.imports.push(token.value);
        this.take(";");
      } else if (this.peek("option")) {
        this.take("option");
        const name = this.parseQualifiedName();
        this.take("=");
        const value = this.take();
        file.options[name] = value.value;
        this.take(";");
      } else if (this.peek("enum")) file.enums.push(this.parseEnum());
      else if (this.peek("message")) file.messages.push(this.parseMessage());
      else throw new Error(`${this.fileName}: unexpected token ${this.take().value}`);
    }
    return file;
  }
}

export function parseProtoSource(source, fileName = "schema.proto") {
  return new Parser(tokenize(source), fileName).parseFile();
}
function camelCase(value) {
  return value.replace(/_([a-z0-9])/g, (_, char) => char.toUpperCase());
}
function pascalCase(value) {
  const camel = camelCase(value);
  return camel.length === 0 ? camel : camel[0].toUpperCase() + camel.slice(1);
}
function snakeCase(value) {
  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/-/g, "_")
    .toLowerCase();
}
function lowerCamel(value) {
  return value.length === 0 ? value : value[0].toLowerCase() + value.slice(1);
}
function baseName(fileName) {
  return fileName
    .split("/")
    .at(-1)
    .replace(/\.proto$/, "");
}
function resolveType(type, file) {
  if (SCALARS.has(type)) return { kind: "scalar", type };
  const normalized = type.startsWith(".") ? type.slice(1) : type;
  if (normalized.includes(".")) return { kind: "named", typeName: normalized };
  return { kind: "named", typeName: `${file.packageName}.${normalized}` };
}
function buildSymbols(files) {
  const symbols = new Map();
  for (const file of files.values()) {
    for (const item of file.enums)
      symbols.set(`${file.packageName}.${item.name}`, { kind: "enum", file, item });
    for (const item of file.messages)
      symbols.set(`${file.packageName}.${item.name}`, { kind: "message", file, item });
  }
  return symbols;
}
function relativeGeneratedImport(fromFile, dependency, extension) {
  const fromParts = fromFile.split("/");
  fromParts.pop();
  const toParts = dependency.replace(/\.proto$/, extension).split("/");
  while (fromParts.length > 0 && toParts.length > 0 && fromParts[0] === toParts[0]) {
    fromParts.shift();
    toParts.shift();
  }
  return `${fromParts.length === 0 ? "./" : "../".repeat(fromParts.length)}${toParts.join("/")}`;
}
function scalarTsType(type) {
  if (["int64", "uint64", "sint64", "fixed64", "sfixed64"].includes(type)) return "bigint";
  if (type === "bool") return "boolean";
  if (type === "string") return "string";
  if (type === "bytes") return "Uint8Array";
  return "number";
}

function generateTypeScript(file, symbols) {
  const js = ["// @generated by protoc-gen-ym-connect. Do not edit."];
  const dts = ["// @generated by protoc-gen-ym-connect. Do not edit."];
  for (const dependency of file.imports)
    js.push(`import ${JSON.stringify(relativeGeneratedImport(file.name, dependency, "_pb.js"))};`);
  const runtimePath = `${"../".repeat(file.name.split("/").length)}runtime.js`;
  js.push(`import { defineEnum, defineMessage } from ${JSON.stringify(runtimePath)};`);
  dts.push(`import type { MessageSchema } from ${JSON.stringify(runtimePath)};`);
  const dependencyTypes = new Map();
  for (const message of file.messages)
    for (const field of message.fields) {
      for (const target of field.map ? [field.map.valueType] : [field.type]) {
        const resolved = resolveType(target, file);
        if (resolved.kind !== "named") continue;
        const symbol = symbols.get(resolved.typeName);
        if (symbol && symbol.file.name !== file.name) {
          const set = dependencyTypes.get(symbol.file.name) ?? new Set();
          set.add(symbol.item.name);
          dependencyTypes.set(symbol.file.name, set);
        }
      }
    }
  for (const [dependency, names] of [...dependencyTypes.entries()].sort()) {
    dts.push(
      `import type { ${[...names].sort().join(", ")} } from ${JSON.stringify(relativeGeneratedImport(file.name, dependency, "_pb.js"))};`,
    );
  }
  js.push("");
  dts.push("");
  for (const enumeration of file.enums) {
    const values = enumeration.values
      .map((value) => `${JSON.stringify(value.name)}: ${value.number}`)
      .join(", ");
    js.push(
      `export const ${enumeration.name} = defineEnum(${JSON.stringify(`${file.packageName}.${enumeration.name}`)}, { ${values} });`,
    );
    dts.push(`export declare const ${enumeration.name}: Readonly<{`);
    for (const value of enumeration.values) dts.push(`  readonly ${value.name}: ${value.number};`);
    dts.push("}>;");
    dts.push(
      `export type ${enumeration.name} = (typeof ${enumeration.name})[keyof typeof ${enumeration.name}];`,
      "",
    );
  }
  for (const message of file.messages) {
    dts.push(`export interface ${message.name} {`);
    const oneofFields = new Map();
    for (const field of message.fields) {
      if (field.oneof) {
        const group = oneofFields.get(field.oneof) ?? [];
        group.push(field);
        oneofFields.set(field.oneof, group);
        continue;
      }
      let type;
      if (field.map) {
        const valueResolved = resolveType(field.map.valueType, file);
        const valueType =
          valueResolved.kind === "scalar"
            ? scalarTsType(valueResolved.type)
            : valueResolved.typeName.split(".").at(-1);
        type = `Record<string, ${valueType}>`;
      } else {
        const resolved = resolveType(field.type, file);
        type =
          resolved.kind === "scalar"
            ? scalarTsType(resolved.type)
            : resolved.typeName.split(".").at(-1);
        if (field.label === "repeated") type = `${type}[]`;
        else if (resolved.kind === "named" && symbols.get(resolved.typeName)?.kind === "message")
          type += " | undefined";
      }
      dts.push(`  ${camelCase(field.name)}: ${type};`);
    }
    for (const [oneofName, fields] of oneofFields) {
      const alternatives = fields.map((field) => {
        const resolved = resolveType(field.type, file);
        const type =
          resolved.kind === "scalar"
            ? scalarTsType(resolved.type)
            : resolved.typeName.split(".").at(-1);
        return `{ case: ${JSON.stringify(camelCase(field.name))}; value: ${type} }`;
      });
      alternatives.push("{ case: undefined; value?: undefined }");
      dts.push(`  ${camelCase(oneofName)}: ${alternatives.join(" | ")};`);
    }
    dts.push("}");
    const fieldSpecs = message.fields.map((field) => {
      if (field.map) {
        const valueResolved = resolveType(field.map.valueType, file);
        const valueSpec =
          valueResolved.kind === "scalar"
            ? `{ kind: "scalar", scalar: ${JSON.stringify(valueResolved.type)} }`
            : `{ kind: ${JSON.stringify(symbols.get(valueResolved.typeName)?.kind ?? "message")}, typeName: ${JSON.stringify(valueResolved.typeName)} }`;
        return `{ no: ${field.number}, name: ${JSON.stringify(camelCase(field.name))}, protoName: ${JSON.stringify(field.name)}, kind: "map", keyScalar: ${JSON.stringify(field.map.keyType)}, value: ${valueSpec} }`;
      }
      const resolved = resolveType(field.type, file);
      const typeSpec =
        resolved.kind === "scalar"
          ? `kind: "scalar", scalar: ${JSON.stringify(resolved.type)}`
          : `kind: ${JSON.stringify(symbols.get(resolved.typeName)?.kind ?? "message")}, typeName: ${JSON.stringify(resolved.typeName)}`;
      return `{ no: ${field.number}, name: ${JSON.stringify(camelCase(field.name))}, protoName: ${JSON.stringify(field.name)}, ${typeSpec}${field.label === "repeated" ? ", repeated: true" : ""}${field.oneof ? `, oneof: ${JSON.stringify(camelCase(field.oneof))}` : ""} }`;
    });
    js.push(
      `export const ${message.name}Schema = defineMessage(${JSON.stringify(`${file.packageName}.${message.name}`)}, [`,
    );
    for (const fieldSpec of fieldSpecs) js.push(`  ${fieldSpec},`);
    js.push("]);", "");
    dts.push(`export declare const ${message.name}Schema: MessageSchema<${message.name}>;`, "");
  }
  return [
    { name: file.name.replace(/\.proto$/, "_pb.js"), content: `${js.join("\n")}\n` },
    { name: file.name.replace(/\.proto$/, "_pb.d.ts"), content: `${dts.join("\n")}\n` },
  ];
}

function rustScalar(type) {
  return {
    double: "f64",
    float: "f32",
    int32: "i32",
    int64: "i64",
    uint32: "u32",
    uint64: "u64",
    sint32: "i32",
    sint64: "i64",
    fixed32: "u32",
    fixed64: "u64",
    sfixed32: "i32",
    sfixed64: "i64",
    bool: "bool",
    string: "::prost::alloc::string::String",
    bytes: "::prost::alloc::vec::Vec<u8>",
  }[type];
}
function generateRust(file, symbols) {
  const out = ["// @generated by protoc-gen-ym-connect. Do not edit.", ""];
  for (const enumeration of file.enums) {
    out.push(
      "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]",
      "#[repr(i32)]",
      `pub enum ${enumeration.name} {`,
    );
    for (const value of enumeration.values)
      out.push(`    ${pascalCase(value.name.toLowerCase())} = ${value.number},`);
    out.push("}", "");
  }
  for (const message of file.messages) {
    out.push("#[derive(Clone, PartialEq, ::prost::Message)]", `pub struct ${message.name} {`);
    const oneofGroups = new Map();
    for (const field of message.fields) {
      if (field.oneof) {
        const group = oneofGroups.get(field.oneof) ?? [];
        group.push(field);
        oneofGroups.set(field.oneof, group);
        continue;
      }
      let rustType;
      let attr;
      if (field.map) {
        const valueResolved = resolveType(field.map.valueType, file);
        const valueType =
          valueResolved.kind === "scalar"
            ? rustScalar(valueResolved.type)
            : valueResolved.typeName.split(".").at(-1);
        rustType = `::std::collections::BTreeMap<::prost::alloc::string::String, ${valueType}>`;
        const valueKind = valueResolved.kind === "scalar" ? valueResolved.type : "message";
        attr = `#[prost(btree_map = "string, ${valueKind}", tag = "${field.number}")]`;
      } else {
        const resolved = resolveType(field.type, file);
        if (field.label === "repeated") {
          const element =
            resolved.kind === "scalar"
              ? rustScalar(resolved.type)
              : symbols.get(resolved.typeName)?.kind === "enum"
                ? "i32"
                : resolved.typeName.split(".").at(-1);
          rustType = `::prost::alloc::vec::Vec<${element}>`;
          attr =
            resolved.kind === "scalar"
              ? `#[prost(${resolved.type}, repeated, tag = "${field.number}")]`
              : symbols.get(resolved.typeName)?.kind === "enum"
                ? `#[prost(enumeration = "${resolved.typeName.split(".").at(-1)}", repeated, tag = "${field.number}")]`
                : `#[prost(message, repeated, tag = "${field.number}")]`;
        } else if (resolved.kind === "scalar") {
          rustType = rustScalar(resolved.type);
          attr = `#[prost(${resolved.type}, tag = "${field.number}")]`;
        } else if (symbols.get(resolved.typeName)?.kind === "enum") {
          rustType = "i32";
          attr = `#[prost(enumeration = "${resolved.typeName.split(".").at(-1)}", tag = "${field.number}")]`;
        } else {
          rustType = `::core::option::Option<${resolved.typeName.split(".").at(-1)}>`;
          attr = `#[prost(message, optional, tag = "${field.number}")]`;
        }
      }
      out.push(`    ${attr}`, `    pub ${snakeCase(field.name)}: ${rustType},`);
    }
    for (const [groupName, fields] of oneofGroups) {
      out.push(
        `    #[prost(oneof = "${snakeCase(message.name)}::${pascalCase(groupName)}", tags = "${fields.map((field) => field.number).join(", ")}")]`,
      );
      out.push(
        `    pub ${snakeCase(groupName)}: ::core::option::Option<${snakeCase(message.name)}::${pascalCase(groupName)}>,`,
      );
    }
    out.push("}", "");
    if (oneofGroups.size > 0) {
      out.push(`pub mod ${snakeCase(message.name)} {`);
      for (const [groupName, fields] of oneofGroups) {
        out.push(
          "    #[derive(Clone, PartialEq, ::prost::Oneof)]",
          `    pub enum ${pascalCase(groupName)} {`,
        );
        for (const field of fields) {
          const resolved = resolveType(field.type, file);
          const symbol = resolved.kind === "named" ? symbols.get(resolved.typeName) : undefined;
          const type =
            resolved.kind === "scalar"
              ? rustScalar(resolved.type)
              : symbol?.kind === "enum"
                ? "i32"
                : `super::${resolved.typeName.split(".").at(-1)}`;
          const attr =
            resolved.kind === "scalar"
              ? resolved.type
              : symbol?.kind === "enum"
                ? `enumeration = "super::${resolved.typeName.split(".").at(-1)}"`
                : "message";
          out.push(
            `        #[prost(${attr}, tag = "${field.number}")]`,
            `        ${pascalCase(field.name)}(${type}),`,
          );
        }
        out.push("    }");
      }
      out.push("}", "");
    }
  }
  return [{ name: `${baseName(file.name)}.rs`, content: `${out.join("\n")}\n` }];
}

function generateKotlin(file) {
  const outer = file.options.java_outer_classname ?? pascalCase(baseName(file.name));
  const packageName = file.options.java_package ?? file.packageName;
  const lines = [
    "// @generated by protoc-gen-ym-connect. Do not edit.",
    `package ${packageName}`,
    "",
  ];
  for (const message of file.messages) {
    lines.push(
      `public inline fun ${lowerCamel(message.name)}(block: ${outer}.${message.name}.Builder.() -> Unit = {}): ${outer}.${message.name} =`,
    );
    lines.push(`    ${outer}.${message.name}.newBuilder().apply(block).build()`, "");
  }
  return [
    {
      name: `${packageName.replaceAll(".", "/")}/${pascalCase(baseName(file.name))}Builders.kt`,
      content: `${lines.join("\n")}\n`,
    },
  ];
}

export function generateLanguage(language, files, fileNames) {
  const symbols = buildSymbols(files);
  const outputs = [];
  for (const fileName of fileNames) {
    const file = files.get(fileName);
    if (!file) throw new Error(`requested file not loaded: ${fileName}`);
    if (language === "typescript") outputs.push(...generateTypeScript(file, symbols));
    else if (language === "rust") outputs.push(...generateRust(file, symbols));
    else if (language === "kotlin") outputs.push(...generateKotlin(file));
    else throw new Error(`unsupported language ${language}`);
  }
  return outputs.sort((a, b) => a.name.localeCompare(b.name));
}

function readVarint(bytes, state) {
  let value = 0n;
  let shift = 0n;
  while (state.offset < bytes.length) {
    const byte = bytes[state.offset++];
    if (shift === 63n && byte > 1) throw new Error("varint exceeds 64-bit range");
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) return value;
    shift += 7n;
    if (shift > 63n) throw new Error("varint exceeds 10 bytes");
  }
  throw new Error("truncated varint");
}
function readLengthDelimited(bytes, state) {
  const length = Number(readVarint(bytes, state));
  const end = state.offset + length;
  if (!Number.isSafeInteger(length) || end > bytes.length)
    throw new Error("invalid length-delimited field");
  const value = bytes.subarray(state.offset, end);
  state.offset = end;
  return value;
}
function skipWire(bytes, state, wireType) {
  if (wireType === 0) readVarint(bytes, state);
  else if (wireType === 1) state.offset += 8;
  else if (wireType === 2) readLengthDelimited(bytes, state);
  else if (wireType === 5) state.offset += 4;
  else throw new Error(`unsupported wire type ${wireType}`);
  if (state.offset > bytes.length) throw new Error("truncated field");
}
function decodeRequest(bytes) {
  const state = { offset: 0 };
  const fileNames = [];
  let parameter = "";
  while (state.offset < bytes.length) {
    const tag = Number(readVarint(bytes, state));
    const field = tag >>> 3;
    const wire = tag & 7;
    if (field === 1 && wire === 2)
      fileNames.push(new TextDecoder().decode(readLengthDelimited(bytes, state)));
    else if (field === 2 && wire === 2)
      parameter = new TextDecoder().decode(readLengthDelimited(bytes, state));
    else skipWire(bytes, state, wire);
  }
  return { fileNames, parameter };
}
function encodeVarint(value) {
  let current = BigInt(value);
  const out = [];
  while (current >= 0x80n) {
    out.push(Number((current & 0x7fn) | 0x80n));
    current >>= 7n;
  }
  out.push(Number(current));
  return out;
}
function encodeField(number, content) {
  const bytes = new TextEncoder().encode(content);
  return [...encodeVarint((number << 3) | 2), ...encodeVarint(bytes.length), ...bytes];
}
function encodeResponse(files, error = null) {
  const out = [];
  if (error !== null) out.push(...encodeField(1, error));
  for (const file of files) {
    const nested = [...encodeField(1, file.name), ...encodeField(15, file.content)];
    out.push(...encodeVarint((15 << 3) | 2), ...encodeVarint(nested.length), ...nested);
  }
  return Uint8Array.from(out);
}
async function loadFiles(fileNames) {
  const root = resolve(process.env.YM_CONNECT_ROOT ?? process.cwd());
  const protoRoot = resolve(root, "shared/protocol/proto");
  const loaded = new Map();
  const queue = [...fileNames];
  while (queue.length > 0) {
    const fileName = queue.shift();
    if (loaded.has(fileName)) continue;
    const path = resolve(protoRoot, fileName);
    const relativePath = relative(protoRoot, path);
    if (relativePath === "" || relativePath.startsWith("..") || isAbsolute(relativePath)) {
      throw new Error(`schema path escapes protocol root: ${fileName}`);
    }
    const file = parseProtoSource(await readFile(path, "utf8"), fileName);
    loaded.set(fileName, file);
    for (const dependency of file.imports) queue.push(dependency);
  }
  return loaded;
}
export async function run() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  try {
    const request = decodeRequest(Buffer.concat(chunks));
    const options = Object.fromEntries(
      request.parameter
        .split(",")
        .filter(Boolean)
        .map((part) => {
          const index = part.indexOf("=");
          return index === -1 ? [part, "true"] : [part.slice(0, index), part.slice(index + 1)];
        }),
    );
    if (!options.language) throw new Error("generator option language is required");
    const files = await loadFiles(request.fileNames);
    process.stdout.write(
      encodeResponse(generateLanguage(options.language, files, request.fileNames)),
    );
  } catch (error) {
    const message = error instanceof Error ? (error.stack ?? error.message) : String(error);
    process.stdout.write(encodeResponse([], message));
    process.exitCode = 1;
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await run();
}
