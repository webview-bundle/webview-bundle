plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    `maven-publish`
}

group = "dev.wvb"
version = System.getenv("WVB_VERSION") ?: "0.0.0"

android {
    namespace = "dev.wvb.webview"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

dependencies {
    // Re-exported so consumers get the generated UniFFI bindings (dev.wvb.*)
    // and the bundled native libraries transitively.
    api(project(":lib-android"))
    implementation(libs.kotlinx.coroutines.android)
}

publishing {
    publications {
        register<MavenPublication>("release") {
            artifactId = "webview-bundle"
            afterEvaluate {
                from(components["release"])
            }
            pom {
                name.set("WebViewBundle for Android")
                description.set("System WebView integration for WebViewBundle resources.")
                url.set("https://github.com/webview-bundle/webview-bundle")
            }
        }
    }
}
