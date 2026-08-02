plugins {
    id("com.android.application") version "9.3.1" apply false
    id("com.google.protobuf") version "0.10.0" apply false
}

tasks.register("verifyRepositoryVersion") {
    group = "verification"
    description = "Verifies that root repository version declarations agree."

    doLast {
        val canonical = file("VERSION").readText().trim()
        val packageVersion = providers.fileContents(layout.projectDirectory.file("package.json"))
            .asText.get()
            .lineSequence()
            .first { it.trimStart().startsWith("\"version\"") }
            .substringAfter(':')
            .trim()
            .trimEnd(',')
            .trim('"')
        val cargoVersion = providers.fileContents(layout.projectDirectory.file("Cargo.toml"))
            .asText.get()
            .lineSequence()
            .dropWhile { it.trim() != "[workspace.package]" }
            .drop(1)
            .first { it.trimStart().startsWith("version =") }
            .substringAfter('=')
            .trim()
            .trim('"')

        check(canonical == packageVersion) {
            "VERSION ($canonical) does not match package.json ($packageVersion)."
        }
        check(canonical == cargoVersion) {
            "VERSION ($canonical) does not match Cargo.toml ($cargoVersion)."
        }
    }
}
