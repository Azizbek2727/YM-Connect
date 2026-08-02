# YM Connect protocol — Kotlin

This project adds idiomatic Kotlin builder functions to the canonical generated Java Lite
messages. The functions preserve Java interoperability and do not create a second model layer.

```kotlin
val version = protocolVersion {
    major = 1
    minor = 0
    patch = 0
}
```

Build and test from the repository root:

```bash
./gradlew -p shared/generated/kotlin build
```
