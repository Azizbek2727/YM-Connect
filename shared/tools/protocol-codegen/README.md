# YM Connect protocol code generator

This package provides the repository-owned `protoc-gen-ym-connect` plugin used
by `shared/protocol/buf.gen.yaml`.

The generator deliberately has no third-party runtime dependencies. It reads a
standard `CodeGeneratorRequest`, parses the canonical source files embedded in
the request, and writes a standard `CodeGeneratorResponse` containing:

- dependency-free TypeScript-compatible JavaScript models and declarations;
- `prost`-compatible Rust message and enum definitions;
- Kotlin builder extensions for the generated lite Java messages.

Generation is deterministic: inputs are sorted, map keys are emitted in stable
order, and generated headers identify the owning plugin. Generated files remain
committed so consumers do not need the generator at runtime.
