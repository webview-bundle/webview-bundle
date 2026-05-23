plugins {
    alias(libs.plugins.kotlin.jvm)
    `java-library`
}

dependencies {
    implementation(libs.jna)
    implementation(libs.kotlinx.coroutines.core)
    testImplementation("org.jetbrains.kotlin:kotlin-test")
    testImplementation(libs.junit.jupiter.engine)
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

tasks.named<Test>("test") {
    useJUnitPlatform()
    val nativeLibDir = layout.projectDirectory.dir("jniLibsForLocalTests").asFile.canonicalPath
    systemProperty("jna.library.path", nativeLibDir)
}
