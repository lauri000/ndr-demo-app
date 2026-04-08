import java.util.Properties
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val ndkVersionValue = "26.3.11579264"
val rustAppDir = rootProject.file("rust")
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
        ?: error("Android SDK path was not found. Define sdk.dir in local.properties.")
val androidNdkDir = file("$androidSdkDir/ndk/$ndkVersionValue")
val cargoBinary = file("${System.getProperty("user.home")}/.cargo/bin/cargo")
val uniffiBindgenBinary = file("${System.getProperty("user.home")}/.cargo/bin/uniffi-bindgen")
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
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        testInstrumentationRunnerArguments["clearPackageData"] = "true"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
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
    description = "Generate Kotlin bindings from the app-side Rust UniFFI crate."
    dependsOn(buildRustHostDebug)
    workingDir = rustAppDir
    doFirst {
        generatedUniffiDir.get().asFile.deleteRecursively()
        generatedUniffiDir.get().asFile.mkdirs()
    }
    commandLine(
        uniffiBindgenBinary.absolutePath,
        "generate",
        "--library",
        hostLibraryFile.absolutePath,
        "--language",
        "kotlin",
        "--no-format",
        "--out-dir",
        generatedUniffiDir.get().asFile.absolutePath,
    )
    inputs.file(rustAppDir.resolve("uniffi.toml"))
    inputs.file(hostLibraryFile)
    outputs.dir(generatedUniffiDir)
}

val buildRustAndroid by tasks.registering(Exec::class) {
    group = "rust"
    description = "Build the Android Rust app core library for arm64-v8a devices."
    workingDir = rustAppDir
    doFirst {
        generatedJniDir.get().asFile.deleteRecursively()
        generatedJniDir.get().asFile.mkdirs()
    }
    environment("ANDROID_HOME", androidSdkDir)
    environment("ANDROID_SDK_ROOT", androidSdkDir)
    environment("ANDROID_NDK_HOME", androidNdkDir.absolutePath)
    commandLine(
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
    inputs.file(rustManifestPath)
    inputs.file(rustAppDir.resolve("uniffi.toml"))
    inputs.dir(rustSourceDir)
    outputs.dir(generatedJniDir)
}

buildRustAndroid.configure {
    mustRunAfter(generateRustBindings)
}

tasks.withType<KotlinCompile>().configureEach {
    dependsOn(generateRustBindings)
    source(generatedUniffiDir.get().asFile)
}

tasks.named("preBuild").configure {
    dependsOn(buildRustAndroid)
    dependsOn(generateRustBindings)
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
