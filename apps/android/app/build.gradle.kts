// L'app KyberNow Link : une seule app pour le téléphone, la tablette, le
// téléviseur et la console portable. La bibliothèque native (libkybernow.so,
// cœur Rust et pont JNI) vient de scripts/build-native.sh, dans
// src/main/jniLibs/<abi>/.

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// Même numéro de version que tout KyberNow (apps/Cargo.toml).
val kyberNowVersion: String = rootDir.resolve("../Cargo.toml").readLines()
    .first { it.trim().startsWith("version = ") }
    .substringAfter('"').substringBefore('"')
val versionParts = kyberNowVersion.split('.').map { it.toIntOrNull() ?: 0 }

android {
    namespace = "fr.leslignesclaires.kybernow"
    compileSdk = 36

    defaultConfig {
        applicationId = "fr.leslignesclaires.kybernow"
        minSdk = 30
        targetSdk = 36
        versionCode = versionParts[0] * 10000 + versionParts[1] * 100 + versionParts[2]
        versionName = kyberNowVersion
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    // Signature de publication : keystore donné par l'environnement
    // (scripts/release.sh) ; sans lui, l'APK de publication n'est pas signé.
    val keystore = System.getenv("KYBERNOW_ANDROID_KEYSTORE")
    if (keystore != null) {
        signingConfigs.create("release") {
            storeFile = file(keystore)
            storePassword = System.getenv("KYBERNOW_ANDROID_KEYSTORE_PASSWORD")
            keyAlias = System.getenv("KYBERNOW_ANDROID_KEY_ALIAS") ?: "kybernow"
            keyPassword = System.getenv("KYBERNOW_ANDROID_KEY_PASSWORD") ?: System.getenv("KYBERNOW_ANDROID_KEYSTORE_PASSWORD")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (keystore != null) signingConfig = signingConfigs.getByName("release")
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
        jniLibs.useLegacyPackaging = false
    }
}

kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.tv.material)
    implementation(libs.tvprovider)
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.camera.view)
    implementation(libs.zxing.core)
}
