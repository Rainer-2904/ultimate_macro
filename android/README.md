# Android build and scanner

The Android app hosts the same Rust/Slint UI as desktop in `MainActivity`
(a `NativeActivity`). ZXing Android Embedded 4.3.0 supplies an in-app camera
screen and decodes product barcodes locally, without Google Play Services.
Open Food Facts requires internet only when the product is not cached.

## Build

Requirements: JDK 17 (the wrapper downloads Gradle 8.9 for Android Gradle Plugin 8.7.3),
Android SDK platform 35/build tools, an Android NDK (r28 or newer recommended),
and the Rust Android target. A pinned Gradle wrapper is included.

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
export JAVA_HOME=/path/to/jdk-17
export ANDROID_HOME=/path/to/Android/Sdk
export ANDROID_NDK_HOME=/path/to/Android/Sdk/ndk/<installed-version>
bash android/build.sh
```

`build.sh` defaults to the toolchain under `~/Android`; set the variables above
when using another installation. The Gradle `preBuild` task invokes `cargo ndk` to produce the Rust shared library,
then packages it with the Java host and bundled scanner. The initial configuration
builds **arm64-v8a**, minimum Android **8.0 / API 26**.

```sh
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.rainer.ultimatemacro/.MainActivity
```

For an x86_64 emulator, add `x86_64-linux-android` with rustup, add `x86_64` to
Gradle's ABI filters, and add `-t x86_64` to its cargo-ndk command.
Release signing and Play Store distribution are not configured.

## Data and flow

- Home **+** opens the scanner form and, on Android, launches the camera.
- Permission denial/cancellation returns to manual entry; no meal is recorded.
- A decoded barcode looks in SQLite first, then Open Food Facts, and caches the result.
- The user supplies grams and presses **Add to today's log** to record consumption.
- Closing the form invalidates pending lookup results. Scanning alone never logs food.
- Settings, food cache and logs live in `files/ultimate_macro.db` in private app
  storage; `files/app.log` appends across launches for troubleshooting. Desktop uses the launch directory.
- `src/android.rs` calls Java through JNI. `startScan` dispatches to Android's UI
  thread; `takeScanResult` transfers one result back to Slint's event loop.
- Android HTTPS uses Rustls/WebPKI with bundled Mozilla trust roots. Certificate
  chain, expiry and hostname checks stay enabled. Keep `webpki-root-certs` updated
  when releasing the app; OS-installed private CAs and OS revocation checks are
  not used for the public Open Food Facts endpoint. This avoids the platform
  verifier's OCSP failure on the tested device.
- Shared app initialization and settings callbacks now live in `src/lib.rs`;
  `src/main.rs` is the desktop launcher. `src/scanner.rs` owns the form workflow.

## Verification on a device

1. On a fresh install, tap **+**, grant camera permission, scan an EAN/UPC label.
2. Confirm the product, enter grams, and add it. Check one log entry and updated totals.
3. Scan the cached product offline and verify lookup still works.
4. Try permission denial, Android Back, cancelling lookup, and an unknown barcode.
   None should add a food entry or leave the form permanently busy.
5. Save daily goals, force-stop/reopen, and check that goals and food records persist.
6. Check keyboard visibility, portrait/landscape behavior, and background/resume during a scan.

Host tests cover goal persistence/migration and scanner input validation. A passing
host build does not validate the Android JNI bridge, APK packaging, or camera hardware.

References:
- https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/android/
- https://github.com/journeyapps/zxing-android-embedded
- https://github.com/bbqsrc/cargo-ndk
