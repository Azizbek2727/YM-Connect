plugins {
    kotlin("jvm") version "2.4.10"
}

group = "dev.ymconnect"
version = "0.1.0"

kotlin {
    jvmToolchain(17)
}

sourceSets {
    main {
        java.srcDir("../java/src/main/java")
        kotlin.srcDir("src/main/kotlin")
    }
}

dependencies {
    implementation("com.google.protobuf:protobuf-javalite:4.35.1")
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.13.4")
}

tasks.test {
    useJUnitPlatform()
}
