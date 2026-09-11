# Building Android APKs and installing with ADB

This guide is for Ultimate Macro's Rust/Slint Android app. Commands use Bash
on Linux and run from the **repository root**, unless stated otherwise.
ADB installation transfers the APK to a connected device and installs it;
`adb push` alone only copies a file. No app store upload is involved.

## Contents

- [Quick start](#quick-start)
- [Project configuration](#project-configuration)
- [Install and configure the toolchain](#install-and-configure-the-toolchain)
- [Connect a device](#connect-a-device)
- [Build, install, and launch](#build-install-and-launch)
- [Signed release APK](#signed-release-apk)
- [Emulators and other architectures](#emulators-and-other-architectures)
- [Logs, backups, and resets](#logs-backups-and-resets)
- [Troubleshooting](#troubleshooting)
- [Data and flow](#data-and-flow)
- [Verification on a device](#verification-on-a-device)

## Quick start

With the toolchain installed and USB debugging authorized:

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export JAVA_HOME="$HOME/Android/build-tools-host/jdk-17"
export PATH="$JAVA_HOME/bin:$HOME/.cargo/bin:$ANDROID_HOME/platform-tools:$PATH"

adb devices -l
# Replace with the serial printed by adb devices (without angle brackets).
export ANDROID_SERIAL='YOUR_DEVICE_SERIAL'

bash android/build.sh &&
  adb install -r android/app/build/outputs/apk/debug/app-debug.apk &&
  adb shell am force-stop com.rainer.ultimatemacro &&
  adb shell am start -W -n com.rainer.ultimatemacro/.MainActivity
```

Replace `JAVA_HOME` if JDK 17 is installed elsewhere. The build script's defaults
match the paths above, but it does not install those tools. `ANDROID_SERIAL`
selects the device for subsequent ADB commands; use `unset ANDROID_SERIAL` to
clear it. The `&&` chain prevents installation of an older APK if building fails.

## Project configuration

These values come from [app/build.gradle](app/build.gradle),
[build.gradle](build.gradle), [build.sh](build.sh), and the
[manifest](app/src/main/AndroidManifest.xml):

| Item | Current value |
| --- | --- |
| Application ID | `com.rainer.ultimatemacro` |
| Launcher | `com.rainer.ultimatemacro/.MainActivity` |
| Minimum Android | Android 8.0 / API 26 |
| Compile / target SDK | 35 / 35 |
| Packaged architecture | `arm64-v8a` only |
| Rust target | `aarch64-linux-android` |
| Rust shared library | `libultimate_macro.so` |
| Java | JDK 17 |
| Gradle wrapper / Android Gradle Plugin | 8.9 / 8.7.3 |
| Script's default NDK | `28.2.13676358` (r28c) |
| App version | `versionCode 1`, `versionName 0.1.0` |
| Release signing | Not configured in Gradle |

`preBuild` depends on `buildRust`. That task runs `cargo ndk` with API 26,
`--lib --release --locked`, writes libraries to `app/src/main/jniLibs`, and sets
`ANDROID_JAR` to the SDK 35 platform jar for Slint's Java helper. Gradle then
packages the library, Java `NativeActivity`, resources, and ZXing scanner.
**Both debug and release APKs currently use the Rust release profile.** An
Android debug APK is still debuggable and automatically debug-signed.

## Install and configure the toolchain

1. Install Rust using [rustup](https://rustup.rs/) and a JDK 17 distribution.
   The crate uses Rust edition 2024; use a toolchain that also satisfies the
   dependencies in `Cargo.lock`. The repository does not pin Rust itself.
2. Install Android Studio's SDK tools, or download the Android command-line
   tools from the [Android Studio downloads page](https://developer.android.com/studio).
   For a manual installation, arrange them so
   `$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager` exists.
3. Export the variables in the quick start, adjusting paths to your installation.
   Put them in your shell startup file if desired. If `ANDROID_SDK_ROOT` is
   already set, keep it consistent with `ANDROID_HOME`.
4. Install SDK packages and accept the licenses interactively:

```bash
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --sdk_root="$ANDROID_HOME" \
  "platform-tools" "platforms;android-35" "build-tools;34.0.0" \
  "ndk;28.2.13676358"
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --sdk_root="$ANDROID_HOME" --licenses
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
```

SDK platform 35 and Build Tools 34.0.0 are distinct packages. AGP 8.7 defaults
to Build Tools 34.0.0; the project does not override that version.
See the [AGP 8.7 compatibility table](https://developer.android.com/build/releases/agp-8-7-0-release-notes)
and [sdkmanager instructions](https://developer.android.com/tools/sdkmanager).
`cargo-ndk` uses the selected NDK for cross-compilation; see its
[upstream setup instructions](https://github.com/bbqsrc/cargo-ndk).

Check the installation:

```bash
java -version
cargo --version
cargo ndk --version
rustup target list --installed
test -f "$ANDROID_HOME/platforms/android-35/android.jar"
test -f "$ANDROID_NDK_HOME/source.properties"
adb version
bash android/build.sh --version
```

Expect Java 17, the Android Rust target, and Gradle 8.9. A failed `test` indicates
a missing path. The first build needs internet for Gradle, Maven dependencies,
and Rust crates. Use the included wrapper; no global Gradle install is needed.
Android Studio users should open the `android/` directory and ensure its Gradle
JDK and environment match this configuration.

## Connect a device

### USB

Enable Developer options (usually by tapping **Build number** seven times),
then enable **USB debugging**. Connect a data-capable cable, unlock the device,
and accept the computer's RSA authorization prompt. Run:

```bash
adb devices -l
```

The device must appear with state `device`. On Linux, USB access may require
udev rules and group membership; Windows may require an OEM USB driver.
Follow the [hardware device setup instructions](https://developer.android.com/studio/run/device)
for your host OS. Avoid running ADB as root to work around USB permissions.

Select a device and check project compatibility:

```bash
export ANDROID_SERIAL='YOUR_DEVICE_SERIAL'
adb shell getprop ro.build.version.sdk
adb shell getprop ro.product.cpu.abilist
```

The SDK must be at least `26`; the ABI list must contain `arm64-v8a` for the
unmodified project. Alternatively, prefix individual commands with
`adb -s YOUR_DEVICE_SERIAL` instead of exporting `ANDROID_SERIAL`.

### Wi-Fi (Android 11+ phones)

On the same Wi-Fi network, enable **Developer options → Wireless debugging**,
then select **Pair device with pairing code**. Replace the example addresses:

```bash
adb pair 192.168.1.50:37123
# Enter the pairing code at the prompt.
adb connect 192.168.1.50:40123
adb devices -l
export ANDROID_SERIAL='192.168.1.50:40123'
```

Use the pairing dialog's port for `pair` and the main Wireless debugging
screen's connection port for `connect`; they can differ. Select the serial
actually shown in `adb devices`. Ports may change after reconnection. If pairing
succeeds but connection fails, check Wi-Fi isolation/firewall settings or use USB.
See the [ADB connection and command reference](https://developer.android.com/tools/adb).

## Build, install, and launch

### Debug APK: normal development

```bash
bash android/build.sh
# Equivalent explicit task:
bash android/build.sh :app:assembleDebug
```

Output: `android/app/build/outputs/apk/debug/app-debug.apk`.
The task only builds; install and launch separately:

```bash
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
adb shell am force-stop com.rainer.ultimatemacro
adb shell am start -W -n com.rainer.ultimatemacro/.MainActivity
adb shell pm path com.rainer.ultimatemacro
```

`-r` replaces an existing compatible installation while retaining its data.
Updates require a matching signing key. Confirm `Success` from installation and
`Status: ok` from launching. `pm path` confirms the package is installed.
Debug signing is automatic; see [command-line Android builds](https://developer.android.com/build/building-cmdline).

For one connected device, Gradle can also build and install in one task:

```bash
bash android/build.sh :app:installDebug
```

Use explicit ADB selection when multiple devices are connected; the guide does
not rely on Gradle honoring `ANDROID_SERIAL`. Rebuild after changing Rust, Slint,
Java, resources, or Gradle configuration; running desktop `cargo build` does not
produce an APK. Rust compilation remains incremental through Cargo.

For build diagnostics or a Gradle clean rebuild:

```bash
bash android/build.sh :app:assembleDebug --stacktrace --info
bash android/build.sh clean :app:assembleDebug
```

Gradle `clean` does not clear Cargo's `target/` cache or the generated
`app/src/main/jniLibs/` source directory. A full `cargo clean` also removes desktop
Rust build artifacts and is normally unnecessary.

## Signed release APK

`assembleRelease` currently produces an **unsigned** APK. To create a locally
installable release without changing Gradle, build and sign it manually.

First create a signing key once, outside the repository. Use your own identity
when prompted, and keep the keystore and passwords backed up securely; future
updates need the same key. Do not regenerate it for every build.

```bash
mkdir -p "$HOME/.android/ultimate-macro-keys"
keytool -genkeypair -v \
  -keystore "$HOME/.android/ultimate-macro-keys/release.jks" \
  -alias ultimate-macro -keyalg RSA -keysize 2048 -validity 10000
```

Build and align before signing. Build Tools 35.0.0 is installed here explicitly
for the `zipalign -P 16` command; this does not change AGP's build tools selection.

```bash
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --sdk_root="$ANDROID_HOME" "build-tools;35.0.0"
bash android/build.sh :app:assembleRelease
export APK_TOOLS="$ANDROID_HOME/build-tools/35.0.0"
export APK_RELEASE_DIR='android/app/build/outputs/apk/release'

"$APK_TOOLS/zipalign" -P 16 -f -v 4 \
  "$APK_RELEASE_DIR/app-release-unsigned.apk" \
  "$APK_RELEASE_DIR/app-release-aligned.apk" &&
"$APK_TOOLS/apksigner" sign \
  --ks "$HOME/.android/ultimate-macro-keys/release.jks" \
  --ks-key-alias ultimate-macro \
  --out "$APK_RELEASE_DIR/app-release-signed.apk" \
  "$APK_RELEASE_DIR/app-release-aligned.apk" &&
"$APK_TOOLS/apksigner" verify --verbose --print-certs \
  "$APK_RELEASE_DIR/app-release-signed.apk"
```

Proceed only after the build and verification succeed. Passwords are requested
interactively. Do not modify the APK after signing. Alignment of the APK alone
does not establish native-library compatibility with every device page size;
validate on your target hardware. See [zipalign](https://developer.android.com/tools/zipalign)
and [apksigner](https://developer.android.com/tools/apksigner).

```bash
adb install -r android/app/build/outputs/apk/release/app-release-signed.apk
adb shell am start -W -n com.rainer.ultimatemacro/.MainActivity
```

A release key differs from the normal debug key: switching between these builds
under the same application ID requires removing the installed app first, which
**deletes its local data**. Back up a debug installation before switching (below).
Increasing `versionCode` and updating `versionName` in `app/build.gradle` is part
of preparing a new release. Play Store publishing, app bundles, and automated
release signing are outside the current project configuration.

## Emulators and other architectures

An ARM64 emulator can use the existing APK. For an x86_64 emulator:

1. Run `rustup target add x86_64-linux-android`.
2. In `app/build.gradle`, change the filter to
   `ndk { abiFilters 'arm64-v8a', 'x86_64' }`.
3. In that file's `buildRust` command, add `'-t', 'x86_64'` after the existing
   `'-t', 'arm64-v8a'` arguments.
4. Rebuild the debug APK, select the emulator serial from `adb devices -l`, and
   install using the same ADB commands.

Both changes are necessary: Rust must produce the library and Gradle must
package its ABI. This makes a larger APK containing both architectures.
Use an emulator with API 26 or newer. Validate the barcode scanner on physical
camera hardware even when emulator UI checks pass.

## Logs, backups, and resets

The project writes `files/app.log` and `files/ultimate_macro.db` in private app
storage. After launching the app, inspect or export its log on a **debug build**:

```bash
adb shell run-as com.rainer.ultimatemacro cat files/app.log
adb exec-out run-as com.rainer.ultimatemacro cat files/app.log > /tmp/ultimate-macro-app.log
```

For startup crashes, collect Android logs immediately after reproducing:

```bash
adb logcat -b crash -d
adb logcat -d -v threadtime > /tmp/ultimate-macro-logcat.txt
```

Logs may contain personal data; inspect before sharing. For debug data backups,
stop the app and archive the entire files directory, including any SQLite
journal/WAL sidecars. Use a fresh local filename for each backup:

```bash
adb shell am force-stop com.rainer.ultimatemacro
adb exec-out run-as com.rainer.ultimatemacro tar -cf - files > /tmp/ultimate-macro-files.tar
tar -tf /tmp/ultimate-macro-files.tar
```

Confirm ADB succeeded and the archive contains `files/ultimate_macro.db` before
relying on it. `run-as` is normally unavailable on release builds, and the
manifest disables Android backup (`allowBackup="false"`). This project has no
release data export/import workflow. A debug archive can be restored to a
compatible debug installation after stopping it; this overwrites current files:

```bash
adb shell am force-stop com.rainer.ultimatemacro
adb shell run-as com.rainer.ultimatemacro tar -xf - < /tmp/ultimate-macro-files.tar
adb shell am start -W -n com.rainer.ultimatemacro/.MainActivity
```

Use a backup from a compatible database version; restoring across schema changes
requires care. For deliberate testing only, these commands **delete app data**:

```bash
# Reset data and permissions while keeping the installed app:
adb shell pm clear com.rainer.ultimatemacro
# Or remove the app and its data entirely:
adb uninstall com.rainer.ultimatemacro
```

## Troubleshooting

| Symptom | Action |
| --- | --- |
| `java` missing or wrong Java version | Set `JAVA_HOME` to JDK 17 and prepend its `bin` to `PATH`; inspect `bash android/build.sh --version`. |
| SDK location missing | Set `ANDROID_HOME`; check for a stale `sdk.dir` in ignored `android/local.properties`. |
| Missing `android-35/android.jar` | Install `platforms;android-35`; `buildRust` explicitly requires this jar. |
| SDK licenses not accepted | Run the `sdkmanager --licenses` command above interactively. |
| NDK missing / compiler not found | Verify `ANDROID_NDK_HOME` names the installed version directory, not the parent `ndk/`. |
| `no such command: ndk` | Install `cargo-ndk` and ensure `$HOME/.cargo/bin` is on `PATH`. |
| Cannot find crate `std` for Android | Add `aarch64-linux-android` for the active Rust toolchain. |
| Locked dependency resolution fails | Check that `Cargo.toml` and `Cargo.lock` agree; the build intentionally uses `--locked`. |
| Gradle/Maven/Cargo download fails | Check network/proxy access and rerun; offline mode only works with complete caches. |
| `gradlew: Permission denied` | Restore the executable bit with `chmod +x android/gradlew`. |
| No device / `unauthorized` / `offline` | Check cable, debugging authorization, and host USB setup; reconnect and inspect `adb devices -l`. |
| Multiple devices or stale serial | Set `ANDROID_SERIAL` to a currently connected serial, or use `adb -s SERIAL`. |
| `INSTALL_FAILED_NO_MATCHING_ABIS` | Confirm `arm64-v8a` support or add the emulator architecture as above. |
| `INSTALL_FAILED_OLDER_SDK` | Use an API 26+ device. |
| `INSTALL_FAILED_UPDATE_INCOMPATIBLE` | Signing keys differ. Use the original key or back up and deliberately uninstall first. |
| `INSTALL_FAILED_VERSION_DOWNGRADE` | Build with a suitable `versionCode`; avoid uninstalling merely to bypass the error. |
| `INSTALL_PARSE_FAILED_NO_CERTIFICATES` | Install the debug APK or the verified signed release APK, not the unsigned release output. |
| `INSTALL_FAILED_INSUFFICIENT_STORAGE` | Free device storage and retry. |
| Installation blocked by device policy | Check the device's debugging/install restrictions and accept any on-device install prompt. |
| Activity not found | Check install success, selected serial, and the exact launcher component above. |
| App immediately exits / native library load error | Capture crash logs; verify the APK contains `lib/arm64-v8a/libultimate_macro.so` using `unzip -l`. |
| Scanner permission denied | Enable Camera in the app's Android permission settings or use manual barcode entry. |
| `run-as: package not debuggable` | Private-file commands require a debug installation. |

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
