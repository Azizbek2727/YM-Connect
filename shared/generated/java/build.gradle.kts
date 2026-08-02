plugins {
    `java-library`
}

group = "dev.ymconnect"
version = "0.1.0"

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(17))
    }
    withSourcesJar()
}

dependencies {
    api("com.google.protobuf:protobuf-javalite:4.35.1")
    testImplementation("org.junit.jupiter:junit-jupiter:5.13.4")
}

tasks.withType<JavaCompile>().configureEach {
    options.encoding = "UTF-8"
    options.release.set(17)
}

tasks.test {
    useJUnitPlatform()
}
