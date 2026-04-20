import java.time.Instant
import java.util.Properties
import org.gradle.api.tasks.testing.Test
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val ndkVersionValue = "26.3.11579264"
val appVersionCode = 1
val appVersionName = "0.1.0"
val rustAppDir = rootProject.file("../core")
val rustManifestPath = rustAppDir.resolve("Cargo.toml")
val rustSourceDir = rustAppDir.resolve("src")
val generatedJniDir = layout.buildDirectory.dir("generated/jniLibs")
val generatedUniffiDir = layout.buildDirectory.dir("generated/source/uniffi/main/java")
val localProperties =
    Properties().apply {
        val file = rootProject.file("local.properties")
        if (file.exists()) {
            file.inputStream().use(::load)
        }
    }
val androidSdkDir =
    localProperties.getProperty("sdk.dir")
        ?: System.getenv("ANDROID_HOME")
        ?: System.getenv("ANDROID_SDK_ROOT")
        ?: error("Android SDK path was not found. Define sdk.dir in android/local.properties.")
val androidNdkDir = file("$androidSdkDir/ndk/$ndkVersionValue")
val cargoBinary = file("${System.getProperty("user.home")}/.cargo/bin/cargo")
val buildGitSha =
    runCatching {
        providers.exec {
            commandLine("git", "-C", rootProject.rootDir.absolutePath, "rev-parse", "--short=12", "HEAD")
        }.standardOutput.asText.get().trim()
    }.getOrElse { "unknown" }
val buildTimestampUtc = Instant.now().toString()
val publicRelayFallbackCsv = "wss://relay.damus.io,wss://nos.lol,wss://relay.primal.net"

fun configValue(propertyName: String, envName: String): String? =
    localProperties.getProperty(propertyName)?.takeIf { it.isNotBlank() }
        ?: System.getenv(envName)?.takeIf { it.isNotBlank() }

fun stringLiteral(value: String): String =
    "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

data class BuildRelayConfig(
    val relaySetId: String,
    val relaysCsv: String,
    val trustedTestBuild: Boolean,
)

val debugRelayConfig =
    BuildRelayConfig(
        relaySetId = configValue("debug.relaySetId", "NDR_DEBUG_RELAY_SET_ID") ?: "public-dev",
        relaysCsv = configValue("debug.relays", "NDR_DEBUG_RELAYS") ?: publicRelayFallbackCsv,
        trustedTestBuild = false,
    )
val betaRelayConfig =
    BuildRelayConfig(
        relaySetId = configValue("beta.relaySetId", "NDR_BETA_RELAY_SET_ID") ?: "beta-fallback",
        relaysCsv = configValue("beta.relays", "NDR_BETA_RELAYS") ?: publicRelayFallbackCsv,
        trustedTestBuild = true,
    )
val releaseRelayConfig =
    BuildRelayConfig(
        relaySetId = configValue("release.relaySetId", "NDR_RELEASE_RELAY_SET_ID") ?: "public-release",
        relaysCsv = configValue("release.relays", "NDR_RELEASE_RELAYS") ?: publicRelayFallbackCsv,
        trustedTestBuild = false,
    )
val betaSigningStoreFile = configValue("beta.storeFile", "NDR_BETA_KEYSTORE_PATH")
val betaSigningStorePassword = configValue("beta.storePassword", "NDR_BETA_KEYSTORE_PASSWORD")
val betaSigningKeyAlias = configValue("beta.keyAlias", "NDR_BETA_KEY_ALIAS")
val betaSigningKeyPassword = configValue("beta.keyPassword", "NDR_BETA_KEY_PASSWORD")
val hasDedicatedBetaSigning =
    !betaSigningStoreFile.isNullOrBlank() &&
        !betaSigningStorePassword.isNullOrBlank() &&
        !betaSigningKeyAlias.isNullOrBlank() &&
        !betaSigningKeyPassword.isNullOrBlank()
val hostLibraryFile =
    rustAppDir.resolve(
        when {
            System.getProperty("os.name").startsWith("Mac", ignoreCase = true) -> "target/debug/libndr_demo_core.dylib"
            System.getProperty("os.name").startsWith("Windows", ignoreCase = true) -> "target/debug/ndr_demo_core.dll"
            else -> "target/debug/libndr_demo_core.so"
        },
    )

android {
    namespace = "social.innode.ndr.demo"
    compileSdk = 36
    ndkVersion = ndkVersionValue

    defaultConfig {
        applicationId = "social.innode.ndr.demo"
        minSdk = 26
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        testInstrumentationRunnerArguments["clearPackageData"] = "true"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    signingConfigs {
        if (hasDedicatedBetaSigning) {
            create("beta") {
                storeFile = file(betaSigningStoreFile!!)
                storePassword = betaSigningStorePassword
                keyAlias = betaSigningKeyAlias
                keyPassword = betaSigningKeyPassword
            }
        }
    }

    buildTypes {
        debug {
            buildConfigField("String", "BUILD_CHANNEL", stringLiteral("debug"))
            buildConfigField("String", "BUILD_GIT_SHA", stringLiteral(buildGitSha))
            buildConfigField("String", "BUILD_TIMESTAMP_UTC", stringLiteral(buildTimestampUtc))
            buildConfigField("String", "RELAY_SET_ID", stringLiteral(debugRelayConfig.relaySetId))
            buildConfigField("String", "DEFAULT_RELAYS_CSV", stringLiteral(debugRelayConfig.relaysCsv))
            buildConfigField("boolean", "TRUSTED_TEST_BUILD", debugRelayConfig.trustedTestBuild.toString())
        }

        create("beta") {
            initWith(getByName("release"))
            applicationIdSuffix = ".beta"
            versionNameSuffix = "-beta"
            isDebuggable = false
            matchingFallbacks += listOf("release")
            signingConfig =
                if (hasDedicatedBetaSigning) {
                    signingConfigs.getByName("beta")
                } else {
                    signingConfigs.getByName("debug")
                }
            buildConfigField("String", "BUILD_CHANNEL", stringLiteral("beta"))
            buildConfigField("String", "BUILD_GIT_SHA", stringLiteral(buildGitSha))
            buildConfigField("String", "BUILD_TIMESTAMP_UTC", stringLiteral(buildTimestampUtc))
            buildConfigField("String", "RELAY_SET_ID", stringLiteral(betaRelayConfig.relaySetId))
            buildConfigField("String", "DEFAULT_RELAYS_CSV", stringLiteral(betaRelayConfig.relaysCsv))
            buildConfigField("boolean", "TRUSTED_TEST_BUILD", betaRelayConfig.trustedTestBuild.toString())
        }

        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            buildConfigField("String", "BUILD_CHANNEL", stringLiteral("release"))
            buildConfigField("String", "BUILD_GIT_SHA", stringLiteral(buildGitSha))
            buildConfigField("String", "BUILD_TIMESTAMP_UTC", stringLiteral(buildTimestampUtc))
            buildConfigField("String", "RELAY_SET_ID", stringLiteral(releaseRelayConfig.relaySetId))
            buildConfigField("String", "DEFAULT_RELAYS_CSV", stringLiteral(releaseRelayConfig.relaysCsv))
            buildConfigField("boolean", "TRUSTED_TEST_BUILD", releaseRelayConfig.trustedTestBuild.toString())
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    sourceSets["main"].jniLibs.setSrcDirs(listOf(generatedJniDir.get().asFile))
}

val buildRustHostDebug by tasks.registering(Exec::class) {
    group = "rust"
    description = "Build the host Rust library for UniFFI binding generation."
    workingDir = rustAppDir
    environment("NDR_APP_VERSION", appVersionName)
    environment("NDR_BUILD_CHANNEL", "debug")
    environment("NDR_BUILD_GIT_SHA", buildGitSha)
    environment("NDR_BUILD_TIMESTAMP_UTC", buildTimestampUtc)
    environment("NDR_DEFAULT_RELAYS", debugRelayConfig.relaysCsv)
    environment("NDR_RELAY_SET_ID", debugRelayConfig.relaySetId)
    environment("NDR_TRUSTED_TEST_BUILD", debugRelayConfig.trustedTestBuild.toString())
    commandLine(
        cargoBinary.absolutePath,
        "build",
        "--manifest-path",
        rustManifestPath.absolutePath,
    )
    inputs.file(rustManifestPath)
    inputs.file(rustAppDir.resolve("uniffi.toml"))
    inputs.dir(rustSourceDir)
    outputs.file(hostLibraryFile)
}

val generateRustBindings by tasks.registering(Exec::class) {
    group = "rust"
    description = "Generate Kotlin bindings from the shared Rust UniFFI crate."
    dependsOn(buildRustHostDebug)
    workingDir = rustAppDir
    doFirst {
        generatedUniffiDir.get().asFile.deleteRecursively()
        generatedUniffiDir.get().asFile.mkdirs()
    }
    commandLine(
        cargoBinary.absolutePath,
        "run",
        "-q",
        "--manifest-path",
        rustAppDir.resolve("uniffi-bindgen/Cargo.toml").absolutePath,
        "--",
        "generate",
        "--library",
        hostLibraryFile.absolutePath,
        "--language",
        "kotlin",
        "--no-format",
        "--out-dir",
        generatedUniffiDir.get().asFile.absolutePath,
        "--config",
        rustAppDir.resolve("uniffi.toml").absolutePath,
    )
    inputs.file(rustAppDir.resolve("uniffi.toml"))
    inputs.file(hostLibraryFile)
    outputs.dir(generatedUniffiDir)
}

fun registerRustAndroidTask(
    taskName: String,
    descriptionText: String,
    buildChannel: String,
    relayConfig: BuildRelayConfig,
    releaseMode: Boolean,
) =
    tasks.register(taskName, Exec::class) {
        group = "rust"
        description = descriptionText
        workingDir = rustAppDir
        doFirst {
            generatedJniDir.get().asFile.deleteRecursively()
            generatedJniDir.get().asFile.mkdirs()
        }
        environment("ANDROID_HOME", androidSdkDir)
        environment("ANDROID_SDK_ROOT", androidSdkDir)
        environment("ANDROID_NDK_HOME", androidNdkDir.absolutePath)
        environment("NDR_APP_VERSION", appVersionName)
        environment("NDR_BUILD_CHANNEL", buildChannel)
        environment("NDR_BUILD_GIT_SHA", buildGitSha)
        environment("NDR_BUILD_TIMESTAMP_UTC", buildTimestampUtc)
        environment("NDR_DEFAULT_RELAYS", relayConfig.relaysCsv)
        environment("NDR_RELAY_SET_ID", relayConfig.relaySetId)
        environment("NDR_TRUSTED_TEST_BUILD", relayConfig.trustedTestBuild.toString())
        val command =
            mutableListOf(
                cargoBinary.absolutePath,
                "ndk",
                "-t",
                "arm64-v8a",
                "-P",
                "26",
                "-o",
                generatedJniDir.get().asFile.absolutePath,
                "--manifest-path",
                rustManifestPath.absolutePath,
                "build",
            )
        if (releaseMode) {
            command += "--release"
        }
        commandLine(command)
        inputs.file(rustManifestPath)
        inputs.file(rustAppDir.resolve("uniffi.toml"))
        inputs.dir(rustSourceDir)
        outputs.dir(generatedJniDir)
    }

val buildRustAndroidDebug =
    registerRustAndroidTask(
        "buildRustAndroidDebug",
        "Build the Android Rust app core library for debug devices.",
        "debug",
        debugRelayConfig,
        releaseMode = false,
    )
val buildRustAndroidBeta =
    registerRustAndroidTask(
        "buildRustAndroidBeta",
        "Build the Android Rust app core library for beta devices.",
        "beta",
        betaRelayConfig,
        releaseMode = true,
    )
val buildRustAndroidRelease =
    registerRustAndroidTask(
        "buildRustAndroidRelease",
        "Build the Android Rust app core library for release devices.",
        "release",
        releaseRelayConfig,
        releaseMode = true,
    )

listOf(buildRustAndroidDebug, buildRustAndroidBeta, buildRustAndroidRelease).forEach { taskProvider ->
    taskProvider.configure {
        mustRunAfter(generateRustBindings)
    }
}

tasks.withType<KotlinCompile>().configureEach {
    dependsOn(generateRustBindings)
    source(generatedUniffiDir.get().asFile)
}

tasks.withType<Test>().configureEach {
    failOnNoDiscoveredTests = false
}

tasks.named("preBuild").configure {
    dependsOn(generateRustBindings)
}

tasks.configureEach {
    when (name) {
        "mergeDebugJniLibFolders" -> dependsOn(buildRustAndroidDebug)
        "mergeBetaJniLibFolders" -> dependsOn(buildRustAndroidBeta)
        "mergeReleaseJniLibFolders" -> dependsOn(buildRustAndroidRelease)
    }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(platform(libs.androidx.compose.bom))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.material3)
    implementation("androidx.compose.material:material-icons-extended")
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.google.mlkit.barcode.scanning)
    implementation(libs.okhttp)
    implementation(libs.zxing.core)
    implementation("net.java.dev.jna:jna:5.12.0@aar")

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)

    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)

    debugImplementation(libs.androidx.compose.ui.tooling)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}
