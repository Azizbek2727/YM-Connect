# YM Connect protocol — Java

This standalone Gradle project contains the generated Java Lite bindings for YM Connect
protocol major version 1. Android consumes the same source tree, so Java and Kotlin callers use
identical message classes and wire behavior.

```java
Common.ProtocolVersion version = Common.ProtocolVersion.newBuilder()
    .setMajor(1)
    .setMinor(0)
    .setPatch(0)
    .build();
byte[] bytes = version.toByteArray();
```

Build and test from the repository root:

```bash
./gradlew -p shared/generated/java build
```
