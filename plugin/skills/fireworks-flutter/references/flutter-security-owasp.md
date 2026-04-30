# OWASP Mobile Top 10 (2024) — Flutter/Dart Security Reference

> Enterprise-grade security reference mapping every OWASP Mobile Top 10 (2024)
> category to Flutter/Dart-specific risks, code patterns, and mitigations.
>
> **Audience**: Flutter developers, security reviewers, CI/CD pipelines
> **Last updated**: 2026-03-27
> **Spec**: OWASP Mobile Top 10 2024 (https://owasp.org/www-project-mobile-top-10/)

---

## Table of Contents

1. [M1: Improper Credential Usage](#m1-improper-credential-usage)
2. [M2: Inadequate Supply Chain Security](#m2-inadequate-supply-chain-security)
3. [M3: Insecure Authentication/Authorization](#m3-insecure-authenticationauthorization)
4. [M4: Insufficient Input/Output Validation](#m4-insufficient-inputoutput-validation)
5. [M5: Insecure Communication](#m5-insecure-communication)
6. [M6: Inadequate Privacy Controls](#m6-inadequate-privacy-controls)
7. [M7: Insufficient Binary Protections](#m7-insufficient-binary-protections)
8. [M8: Security Misconfiguration](#m8-security-misconfiguration)
9. [M9: Insecure Data Storage](#m9-insecure-data-storage)
10. [M10: Insufficient Cryptography](#m10-insufficient-cryptography)
11. [Common False Positives](#common-false-positives)
12. [Integration Points](#integration-points)
13. [Scanner Scripts Reference](#scanner-scripts-reference)

---

## M1: Improper Credential Usage

**Description**: Hardcoded credentials, API keys, or secrets embedded in source code,
assets, or environment variables that ship with the binary. Also covers improper
storage/transmission of user credentials.

### Flutter-Specific Risk Scenarios

- API keys hardcoded in Dart source (visible via `strings` on the binary)
- Firebase config files (`google-services.json`, `GoogleService-Info.plist`) containing
  unrestricted keys checked into version control
- OAuth client secrets embedded in the app instead of using PKCE
- Storing user passwords in `SharedPreferences` or plain-text files
- `.env` files bundled into assets

### WRONG (Insecure)

```dart
// Hardcoded API key — extractable from the compiled binary
// class ApiConfig { ... teaching example showing a class that would hold hardcoded
//   apiKey, databaseUrl, and secretToken fields. We intentionally do NOT write
//   them here - the point is that ANY literal credential inside your compiled
//   app binary is extractable. Use secure storage instead (see below). }

// Storing credentials in SharedPreferences
Future<void> saveCredentials(String username, String password) async {
  final prefs = await SharedPreferences.getInstance();
  await prefs.setString('username', username);
  await prefs.setString('password', password); // Plain text!
}
```

### RIGHT (Secure)

```dart
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

// Keys fetched at runtime from a secure backend, never in source
class ApiConfig {
  static Future<String> getApiKey() async {
    final response = await _authenticatedClient.get('/config/api-key');
    return response.data['key'] as String;
  }
}

// Credentials in platform-native secure storage
class CredentialStore {
  static const _storage = FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
    iOptions: IOSOptions(
      accessibility: KeychainAccessibility.first_unlock_this_device,
    ),
  );

  static Future<void> saveToken(String token) async {
    await _storage.write(key: 'auth_token', value: token);
  }

  static Future<String?> readToken() async {
    return _storage.read(key: 'auth_token');
  }

  static Future<void> clearAll() async {
    await _storage.deleteAll();
  }
}
```

### Platform-Specific Notes

| Platform | Mechanism | Notes |
|----------|-----------|-------|
| Android  | `EncryptedSharedPreferences` / Android Keystore | Requires API 23+. Set `encryptedSharedPreferences: true`. |
| iOS      | Keychain Services | Set `accessibility` to `.whenUnlockedThisDeviceOnly` for max security. |

### Mitigation Checklist

- [ ] No API keys, tokens, or passwords in Dart source files
- [ ] `google-services.json` and `GoogleService-Info.plist` restricted via Firebase security rules
- [ ] All secrets fetched at runtime from authenticated backend
- [ ] User credentials stored only via `flutter_secure_storage`
- [ ] OAuth flows use PKCE (no client secret in mobile app)
- [ ] `.env` files in `.gitignore` and NOT in `assets/`
- [ ] CI/CD secrets injected at build time via `--dart-define-from-file` (file not committed)

---

## M2: Inadequate Supply Chain Security

**Description**: Risks from third-party dependencies, compromised packages, malicious
plugins, or lack of dependency verification.

### Flutter-Specific Risk Scenarios

- Using unvetted pub.dev packages with native platform code
- No lockfile (`pubspec.lock`) committed, causing non-deterministic builds
- Dependency confusion attacks (private package names squatted on pub.dev)
- Outdated packages with known CVEs
- Plugins requesting excessive platform permissions

### WRONG (Insecure)

```yaml
# pubspec.yaml with loose version constraints
dependencies:
  http: any
  some_unknown_auth_package: ^0.0.1  # 2 downloads, no verified publisher
  crypto_utils:
    git:
      url: https://github.com/random-user/crypto_utils.git  # Unverified source
```

```dart
// Blindly trusting third-party deserialization
import 'package:some_unknown_auth_package/auth.dart';

final user = MagicAuth.login(email, password); // No idea what this does internally
```

### RIGHT (Secure)

```yaml
# pubspec.yaml with pinned, verified dependencies
dependencies:
  http: ^1.2.1     # Verified publisher: dart.dev
  dio: ^5.4.3      # Verified publisher: flutterchina.club
  flutter_secure_storage: ^9.2.2  # Verified publisher

# dependency_overrides only for local development, never in release
```

```dart
// Verify package integrity, audit permissions
// In CI pipeline (not Dart, but part of supply chain):
// flutter pub deps --style=compact
// flutter pub outdated
// dart pub audit (hypothetical, use OSV scanner)

// Always review native permissions a plugin requests:
// Check android/src/main/AndroidManifest.xml in plugin source
// Check ios/Classes/*.m for entitlement usage
```

### Platform-Specific Notes

| Platform | Action |
|----------|--------|
| Android  | Review plugin's `AndroidManifest.xml` for `<uses-permission>`. |
| iOS      | Review plugin's `Info.plist` keys and entitlements. |
| Both     | Run `osv-scanner` against `pubspec.lock` in CI. |

### Mitigation Checklist

- [ ] `pubspec.lock` committed to version control
- [ ] All dependencies use verified publishers on pub.dev
- [ ] No `git:` dependencies pointing to unverified repos
- [ ] `flutter pub outdated` run weekly in CI
- [ ] OSV Scanner or Snyk integrated into CI pipeline
- [ ] Plugin native code reviewed before adoption
- [ ] No `dependency_overrides` in release builds
- [ ] License compliance checked (no GPL in proprietary apps without review)

---

## M3: Insecure Authentication/Authorization

**Description**: Weak authentication mechanisms, missing session management, improper
authorization checks that can be bypassed client-side.

### Flutter-Specific Risk Scenarios

- Authorization logic performed only on the client (Dart code)
- JWT tokens stored without expiration checks
- Biometric auth without server-side challenge
- Missing re-authentication for sensitive operations
- Role checks done in Flutter widgets instead of backend

### WRONG (Insecure)

```dart
// Client-side only authorization — trivially bypassable
class AuthGuard {
  static bool isAdmin(User user) {
    return user.role == 'admin'; // Attacker modifies local storage
  }

  static Widget protectedPage(User user) {
    if (isAdmin(user)) {
      return const AdminPanel();
    }
    return const AccessDenied();
  }
}

// Biometric auth without server challenge
Future<bool> authenticateUser() async {
  final localAuth = LocalAuthentication();
  final didAuth = await localAuth.authenticate(
    localizedReason: 'Verify identity',
  );
  if (didAuth) {
    // Immediately grant access — no server verification
    navigateToSecurePage();
  }
  return didAuth;
}
```

### RIGHT (Secure)

```dart
// Server-authoritative authorization
class SecureAuthGuard {
  final ApiClient _api;
  final CredentialStore _credentials;

  SecureAuthGuard(this._api, this._credentials);

  /// Server validates the token AND returns authorized resources.
  /// Client never decides what user can access.
  Future<AuthResult> verifyAccess(String resource) async {
    final token = await _credentials.readToken();
    if (token == null) return AuthResult.unauthenticated;

    final response = await _api.post('/auth/verify', data: {
      'resource': resource,
      'token': token,
    });

    if (response.statusCode == 403) return AuthResult.forbidden;
    if (response.statusCode == 401) {
      await _credentials.clearAll();
      return AuthResult.unauthenticated;
    }

    return AuthResult.authorized(response.data);
  }
}

// Biometric with server-side challenge-response
Future<bool> authenticateWithBiometric() async {
  final localAuth = LocalAuthentication();

  // Step 1: Get challenge from server
  final challenge = await _api.get('/auth/biometric-challenge');
  final nonce = challenge.data['nonce'] as String;

  // Step 2: Local biometric
  final didAuth = await localAuth.authenticate(
    localizedReason: 'Verify identity',
  );
  if (!didAuth) return false;

  // Step 3: Sign nonce with device-bound key and send back
  final signature = await _deviceKeyStore.sign(nonce);
  final verification = await _api.post('/auth/biometric-verify', data: {
    'nonce': nonce,
    'signature': signature,
    'device_id': await _deviceInfo.getDeviceId(),
  });

  return verification.statusCode == 200;
}
```

### Platform-Specific Notes

| Platform | Notes |
|----------|-------|
| Android  | Use `BiometricPrompt` via `local_auth`. Set `biometricOnly: true` to avoid PIN fallback if policy requires. |
| iOS      | Face ID requires `NSFaceIDUsageDescription` in `Info.plist`. Use `LAPolicy.deviceOwnerAuthenticationWithBiometrics`. |

### Mitigation Checklist

- [ ] All authorization decisions made server-side
- [ ] JWT tokens have short expiry with refresh token rotation
- [ ] Biometric auth uses server-side challenge-response
- [ ] Session tokens rotated after privilege changes
- [ ] Re-authentication required for sensitive ops (payment, password change)
- [ ] Token revocation supported server-side
- [ ] Client-side role checks are cosmetic only (hide UI), never security boundaries

---

## M4: Insufficient Input/Output Validation

**Description**: Failure to validate, sanitize, or encode data at trust boundaries,
leading to injection, XSS in WebViews, path traversal, or data corruption.

### Flutter-Specific Risk Scenarios

- SQL injection via raw queries in `sqflite` / `drift`
- XSS in `WebView` when loading user-controlled HTML
- Path traversal when constructing file paths from user input
- Deep link parameter injection
- Unvalidated JSON deserialization causing type confusion

### WRONG (Insecure)

```dart
// SQL injection via string concatenation
Future<List<Product>> searchProducts(String query) async {
  final db = await openDatabase('app.db');
  // NEVER concatenate user input into SQL
  final results = await db.rawQuery(
    "SELECT * FROM products WHERE name LIKE '%$query%'"
  );
  return results.map(Product.fromMap).toList();
}

// XSS in WebView
class UnsafeWebView extends StatelessWidget {
  final String userContent;
  const UnsafeWebView({required this.userContent});

  @override
  Widget build(BuildContext context) {
    return WebViewWidget(
      controller: WebViewController()
        ..loadHtmlString('<html><body>$userContent</body></html>'),
        // userContent could contain <script> tags
    );
  }
}

// Path traversal
Future<File> getUserFile(String filename) async {
  final dir = await getApplicationDocumentsDirectory();
  return File('${dir.path}/$filename'); // filename could be "../../etc/passwd"
}
```

### RIGHT (Secure)

```dart
// Parameterized queries — immune to SQL injection
Future<List<Product>> searchProducts(String query) async {
  final db = await openDatabase('app.db');
  final results = await db.rawQuery(
    'SELECT * FROM products WHERE name LIKE ?',
    ['%$query%'],
  );
  return results.map(Product.fromMap).toList();
}

// Sanitized WebView content
class SafeWebView extends StatelessWidget {
  final String userContent;
  const SafeWebView({required this.userContent});

  String _sanitize(String input) {
    return input
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#x27;');
  }

  @override
  Widget build(BuildContext context) {
    final safe = _sanitize(userContent);
    return WebViewWidget(
      controller: WebViewController()
        ..setJavaScriptMode(JavaScriptMode.disabled) // Disable JS if not needed
        ..loadHtmlString('<html><body>$safe</body></html>'),
    );
  }
}

// Path traversal prevention
Future<File> getUserFile(String filename) async {
  final dir = await getApplicationDocumentsDirectory();
  final sanitized = path.basename(filename); // Strip directory components
  final filePath = path.join(dir.path, sanitized);
  final resolved = path.canonicalize(filePath);

  // Verify resolved path is still inside the allowed directory
  if (!resolved.startsWith(path.canonicalize(dir.path))) {
    throw SecurityException('Path traversal detected: $filename');
  }

  return File(resolved);
}

// Deep link validation
void handleDeepLink(Uri uri) {
  // Whitelist allowed hosts and schemes
  const allowedHosts = {'myapp.com', 'api.myapp.com'};
  const allowedSchemes = {'https', 'myapp'};

  if (!allowedSchemes.contains(uri.scheme)) {
    throw SecurityException('Disallowed scheme: ${uri.scheme}');
  }
  if (uri.host.isNotEmpty && !allowedHosts.contains(uri.host)) {
    throw SecurityException('Disallowed host: ${uri.host}');
  }

  // Validate and parse parameters with type checking
  final id = int.tryParse(uri.queryParameters['id'] ?? '');
  if (id == null || id <= 0) {
    throw const FormatException('Invalid ID parameter');
  }
}
```

### Platform-Specific Notes

| Platform | Notes |
|----------|-------|
| Android  | Deep links defined in `AndroidManifest.xml` — use `autoVerify="true"` for App Links. |
| iOS      | Universal Links via `apple-app-site-association`. Validate `NSUserActivity` URLs. |

### Mitigation Checklist

- [ ] All SQL queries use parameterized statements
- [ ] WebView JavaScript disabled unless explicitly needed
- [ ] User-generated HTML sanitized or rendered as plain text
- [ ] File paths validated against a base directory (no traversal)
- [ ] Deep link parameters validated with strict type checking
- [ ] JSON deserialization uses typed models with validation (e.g., `freezed`, `json_serializable`)
- [ ] Integer overflow and boundary checks on numeric inputs

---

## M5: Insecure Communication

**Description**: Failure to protect data in transit — missing TLS, improper certificate
validation, cleartext traffic, or lack of certificate pinning.

### Flutter-Specific Risk Scenarios

- HTTP (cleartext) requests on Android without Network Security Config
- Disabling TLS certificate verification in `dio` or `http` for "debugging" left in production
- No certificate pinning allowing MITM with rogue CA
- WebSocket connections over `ws://` instead of `wss://`
- gRPC channels without TLS

### WRONG (Insecure)

```dart
// Disabling certificate verification — MITM wide open
import 'dart:io';
import 'package:dio/dio.dart';

Dio createInsecureClient() {
  final dio = Dio();
  // NEVER do this in production
  (dio.httpClientAdapter as IOHttpClientAdapter).createHttpClient = () {
    final client = HttpClient();
    client.badCertificateCallback = (cert, host, port) => true; // Accepts ALL certs
    return client;
  };
  return dio;
}

// Cleartext HTTP
Future<void> fetchData() async {
  final response = await Dio().get('http://api.example.com/data'); // No TLS
}
```

### RIGHT (Secure)

```dart
import 'dart:io';
import 'dart:typed_data';
import 'package:dio/dio.dart';
import 'package:dio/io.dart';
import 'package:flutter/services.dart';

/// Creates a Dio client with certificate pinning.
Future<Dio> createSecureClient() async {
  final dio = Dio(BaseOptions(
    baseUrl: 'https://api.example.com',
    connectTimeout: const Duration(seconds: 15),
    receiveTimeout: const Duration(seconds: 15),
  ));

  // Load pinned certificate from assets
  final certData = await rootBundle.load('assets/certs/api_example_com.pem');

  (dio.httpClientAdapter as IOHttpClientAdapter).createHttpClient = () {
    final context = SecurityContext(withTrustedRoots: false);
    context.setTrustedCertificatesBytes(certData.buffer.asUint8List());

    final client = HttpClient(context: context);
    // Strict hostname verification (default, but be explicit)
    client.badCertificateCallback = (X509Certificate cert, String host, int port) {
      // Log the failure for monitoring
      _securityLogger.warning('Certificate validation failed for $host:$port');
      return false; // REJECT invalid certificates
    };
    return client;
  };

  // Add interceptor for logging (never log sensitive headers in production)
  dio.interceptors.add(InterceptorsWrapper(
    onRequest: (options, handler) {
      // Ensure no accidental downgrade to HTTP
      if (options.uri.scheme != 'https') {
        handler.reject(DioException(
          requestOptions: options,
          message: 'Only HTTPS connections are allowed',
        ));
        return;
      }
      handler.next(options);
    },
  ));

  return dio;
}

/// SHA-256 public key pinning (more resilient to cert rotation)
Future<Dio> createPinningClient() async {
  final dio = Dio(BaseOptions(baseUrl: 'https://api.example.com'));

  const expectedFingerprints = {
    // Pin the CA or intermediate cert, not the leaf (easier rotation)
    'sha256/YLh1dUR9y6Kja30RrAn7JKnbQG/uEtLMkBgFF2Fuihg=',
    'sha256/Vjs8r4z+80wjNcr1YKepWQboSIRi63WsWXhIMN+eWys=', // Backup pin
  };

  (dio.httpClientAdapter as IOHttpClientAdapter).createHttpClient = () {
    final client = HttpClient();
    client.badCertificateCallback = (cert, host, port) {
      final fingerprint = _sha256Fingerprint(cert);
      return expectedFingerprints.contains(fingerprint);
    };
    return client;
  };

  return dio;
}
```

### Platform-Specific Notes

| Platform | Config | Details |
|----------|--------|---------|
| Android  | Network Security Config | `android/app/src/main/res/xml/network_security_config.xml` — set `cleartextTrafficPermitted="false"`. Pin certificates per domain. |
| iOS      | App Transport Security (ATS) | ATS enforces HTTPS by default. Never set `NSAllowsArbitraryLoads: true` in production `Info.plist`. |

**Android `network_security_config.xml`:**
```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <base-config cleartextTrafficPermitted="false">
        <trust-anchors>
            <certificates src="system" />
        </trust-anchors>
    </base-config>
    <domain-config>
        <domain includeSubdomains="true">api.example.com</domain>
        <pin-set expiration="2025-12-31">
            <pin digest="SHA-256">YLh1dUR9y6Kja30RrAn7JKnbQG/uEtLMkBgFF2Fuihg=</pin>
            <pin digest="SHA-256">Vjs8r4z+80wjNcr1YKepWQboSIRi63WsWXhIMN+eWys=</pin>
        </pin-set>
    </domain-config>
</network-security-config>
```

### Mitigation Checklist

- [ ] All network requests use HTTPS exclusively
- [ ] `badCertificateCallback` never returns `true` unconditionally
- [ ] Certificate or public key pinning implemented for API domains
- [ ] Android NSC blocks cleartext traffic (`cleartextTrafficPermitted="false"`)
- [ ] iOS ATS enabled (no `NSAllowsArbitraryLoads: true`)
- [ ] WebSocket connections use `wss://` only
- [ ] Backup pins configured for certificate rotation

---

## M6: Inadequate Privacy Controls

**Description**: Failure to protect PII, collect excessive data, lack transparency in data
handling, or violate platform privacy expectations.

### Flutter-Specific Risk Scenarios

- Logging PII (names, emails, tokens) to the console in release builds
- Clipboard leaking sensitive data
- Screenshot/screen recording not blocked on sensitive screens
- Collecting device identifiers without consent
- Analytics SDKs transmitting PII without disclosure

### WRONG (Insecure)

```dart
// Logging PII in production
void processPayment(String cardNumber, String cvv, User user) {
  debugPrint('Processing payment for ${user.email} card: $cardNumber');
  // This ends up in device logs accessible via adb logcat
}

// Not clearing sensitive data from memory
class PaymentForm extends StatefulWidget {
  @override
  State<PaymentForm> createState() => _PaymentFormState();
}

class _PaymentFormState extends State<PaymentForm> {
  final _cardController = TextEditingController();

  @override
  void dispose() {
    // Card number persists in memory — controller not cleared
    _cardController.dispose();
    super.dispose();
  }
}
```

### RIGHT (Secure)

```dart
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

// Safe logging — strip PII
class SecureLogger {
  static void log(String message, {Map<String, dynamic>? context}) {
    if (kReleaseMode) {
      // In release: send to crash reporting with PII stripped
      _crashReporter.log(_stripPii(message));
    } else {
      debugPrint('[DEBUG] $message');
    }
  }

  static String _stripPii(String input) {
    // Redact emails
    input = input.replaceAll(
      RegExp(r'[\w.+-]+@[\w-]+\.[\w.]+'),
      '[REDACTED_EMAIL]',
    );
    // Redact card numbers (13-19 digits)
    input = input.replaceAll(
      RegExp(r'\b\d{13,19}\b'),
      '[REDACTED_CARD]',
    );
    return input;
  }
}

// Prevent screenshots on sensitive screens (Android)
class SecureScreenMixin {
  /// Call in initState of sensitive pages
  static Future<void> enableSecureMode() async {
    if (Platform.isAndroid) {
      // Requires flutter_windowmanager or method channel
      await const MethodChannel('com.app/security')
          .invokeMethod('enableSecureFlag');
    }
    // iOS: use UIScreen.captured / UIApplication.userDidTakeScreenshotNotification
  }

  static Future<void> disableSecureMode() async {
    if (Platform.isAndroid) {
      await const MethodChannel('com.app/security')
          .invokeMethod('disableSecureFlag');
    }
  }
}

// Clear sensitive form data
class _SecurePaymentFormState extends State<SecurePaymentForm> {
  final _cardController = TextEditingController();

  @override
  void dispose() {
    // Overwrite controller text before disposing
    _cardController.text = '0' * _cardController.text.length;
    _cardController.clear();
    _cardController.dispose();
    super.dispose();
  }
}
```

### Mitigation Checklist

- [ ] No PII in `debugPrint`, `print`, or `log()` in release builds
- [ ] `FLAG_SECURE` set on Android for sensitive screens
- [ ] Clipboard cleared after paste timeout for sensitive fields
- [ ] Minimum data collection — only what is necessary
- [ ] Privacy policy accessible in-app
- [ ] Analytics events reviewed for accidental PII leakage
- [ ] `kReleaseMode` or `kDebugMode` guards around verbose logging

---

## M7: Insufficient Binary Protections

**Description**: Lack of obfuscation, tamper detection, debugger detection, or reverse
engineering mitigations on the released binary.

### Flutter-Specific Risk Scenarios

- Flutter apps ship readable Dart snapshots by default
- No obfuscation flag — class/method names preserved
- No root/jailbreak detection
- Debug mode accidentally enabled in release builds
- No integrity verification of the APK/IPA

### WRONG (Insecure)

```bash
# Building without obfuscation — all symbol names readable
flutter build apk
flutter build ipa
```

```dart
// No runtime integrity checks
void main() {
  runApp(const MyApp()); // No checks for rooted/jailbroken device
}
```

### RIGHT (Secure)

```bash
# Build with obfuscation and split debug info
flutter build apk \
  --obfuscate \
  --split-debug-info=build/debug-info/ \
  --release

flutter build ipa \
  --obfuscate \
  --split-debug-info=build/debug-info/ \
  --release

# Archive debug symbols for crash symbolication
tar -czf debug-symbols-v1.2.3.tar.gz build/debug-info/
```

```dart
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter_jailbreak_detection/flutter_jailbreak_detection.dart';

/// Runtime security checks — run before showing sensitive screens
class IntegrityChecker {
  static Future<SecurityStatus> check() async {
    final issues = <String>[];

    // 1. Debug mode check
    if (kDebugMode) {
      issues.add('debug_mode');
    }

    // 2. Assertions enabled check (catches profile mode misuse)
    bool assertionsEnabled = false;
    assert(() {
      assertionsEnabled = true;
      return true;
    }());
    if (assertionsEnabled && kReleaseMode) {
      issues.add('assertions_in_release');
    }

    // 3. Root/Jailbreak detection
    try {
      final isJailbroken = await FlutterJailbreakDetection.jailbroken;
      if (isJailbroken) {
        issues.add('rooted_device');
      }
      final isDeveloperMode = await FlutterJailbreakDetection.developerMode;
      if (isDeveloperMode) {
        issues.add('developer_mode');
      }
    } catch (_) {
      issues.add('detection_tampered');
    }

    return SecurityStatus(
      isSecure: issues.isEmpty,
      issues: issues,
    );
  }
}

class SecurityStatus {
  final bool isSecure;
  final List<String> issues;
  const SecurityStatus({required this.isSecure, required this.issues});
}
```

### Platform-Specific Notes

| Platform | Protections |
|----------|-------------|
| Android  | Enable R8/ProGuard for native code. Use Play Integrity API. Sign with upload key (Google manages app signing key). |
| iOS      | App Store performs bitcode optimization. Use DeviceCheck API for attestation. |

### Mitigation Checklist

- [ ] `--obfuscate --split-debug-info` on every release build
- [ ] Debug symbols archived securely (not in the app bundle)
- [ ] Root/jailbreak detection with graceful degradation policy
- [ ] `kReleaseMode` verified at startup
- [ ] App integrity APIs used (Play Integrity / DeviceCheck)
- [ ] ProGuard/R8 rules configured for Android native code

---

## M8: Security Misconfiguration

**Description**: Insecure default settings, unnecessary permissions, debug features
enabled in production, or improper platform configuration.

### Flutter-Specific Risk Scenarios

- `android:debuggable="true"` in release manifest
- Excessive permissions in `AndroidManifest.xml`
- `NSAllowsArbitraryLoads` in iOS `Info.plist`
- Export-enabled activities/services without intent filters
- `allowBackup="true"` exposing app data to `adb backup`
- WebView with JavaScript enabled loading untrusted content

### WRONG (Insecure)

```xml
<!-- AndroidManifest.xml with dangerous misconfigurations -->
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.READ_CONTACTS" />
    <uses-permission android:name="android.permission.CAMERA" />
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />
    <uses-permission android:name="android.permission.READ_PHONE_STATE" />
    <!-- All permissions requested even if only INTERNET is needed -->

    <application
        android:allowBackup="true"
        android:debuggable="true"
        android:usesCleartextTraffic="true"
        android:exported="true">
        <!-- Everything wrong -->
    </application>
</manifest>
```

```xml
<!-- iOS Info.plist with ATS disabled -->
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSAllowsArbitraryLoads</key>
    <true/>
</dict>
```

### RIGHT (Secure)

```xml
<!-- AndroidManifest.xml — minimal permissions, hardened config -->
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <!-- Only request what you actually use -->

    <application
        android:allowBackup="false"
        android:debuggable="false"
        android:usesCleartextTraffic="false"
        android:networkSecurityConfig="@xml/network_security_config"
        android:exported="false">

        <activity
            android:name=".MainActivity"
            android:exported="true"
            android:launchMode="singleTop">
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
```

```xml
<!-- iOS Info.plist — ATS enabled, exception only for specific domain if needed -->
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSExceptionDomains</key>
    <dict>
        <key>legacy-api.example.com</key>
        <dict>
            <key>NSExceptionAllowsInsecureHTTPLoads</key>
            <true/>
            <key>NSExceptionMinimumTLSVersion</key>
            <string>TLSv1.2</string>
        </dict>
    </dict>
</dict>
```

```dart
// Runtime permission requests — just in time, not all at startup
import 'package:permission_handler/permission_handler.dart';

class PermissionService {
  /// Request camera only when user taps "Take Photo"
  static Future<bool> requestCamera() async {
    final status = await Permission.camera.request();
    if (status.isPermanentlyDenied) {
      // Guide user to settings
      await openAppSettings();
      return false;
    }
    return status.isGranted;
  }
}
```

### Mitigation Checklist

- [ ] `android:debuggable="false"` in release builds (Gradle handles this, verify)
- [ ] `android:allowBackup="false"` to prevent `adb backup` data extraction
- [ ] `android:usesCleartextTraffic="false"` in manifest
- [ ] Only INTERNET permission unless others are justified
- [ ] iOS ATS enabled — no global `NSAllowsArbitraryLoads`
- [ ] Permissions requested just-in-time, not at app launch
- [ ] No `android:exported="true"` on components without intent filters
- [ ] WebView JavaScript disabled for static content

---

## M9: Insecure Data Storage

**Description**: Sensitive data stored in locations accessible to other apps, backups,
or device compromise — unencrypted databases, shared preferences, temp files, logs.

### Flutter-Specific Risk Scenarios

- Storing tokens/passwords in `SharedPreferences` (XML on Android, plist on iOS)
- SQLite databases with sensitive data unencrypted on disk
- Cached images/files in world-readable directories
- Sensitive data in app screenshots (task switcher thumbnail)
- Temporary files not cleaned up

### WRONG (Insecure)

```dart
// Tokens in SharedPreferences — plaintext on disk
import 'package:shared_preferences/shared_preferences.dart';

Future<void> storeSession(String accessToken, String refreshToken) async {
  final prefs = await SharedPreferences.getInstance();
  await prefs.setString('access_token', accessToken);
  await prefs.setString('refresh_token', refreshToken);
  await prefs.setString('user_ssn', '123-45-6789'); // PII in plaintext!
}

// Unencrypted SQLite with sensitive data
Future<void> createDb() async {
  final db = await openDatabase('user_data.db');
  await db.execute('''
    CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY,
      name TEXT,
      email TEXT,
      credit_card TEXT,
      password_hash TEXT
    )
  ''');
}
```

### RIGHT (Secure)

```dart
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:sqflite_sqlcipher/sqflite.dart';
import 'package:path/path.dart' as p;

// Tokens in platform-native secure storage
class SecureTokenStore {
  static const _storage = FlutterSecureStorage(
    aOptions: AndroidOptions(
      encryptedSharedPreferences: true,
      keyCipherAlgorithm: KeyCipherAlgorithm.RSA_ECB_OAEPwithSHA_256andMGF1Padding,
      storageCipherAlgorithm: StorageCipherAlgorithm.AES_GCM_NoPadding,
    ),
    iOptions: IOSOptions(
      accessibility: KeychainAccessibility.first_unlock_this_device,
      accountName: 'com.example.app',
    ),
  );

  static Future<void> storeTokens({
    required String accessToken,
    required String refreshToken,
  }) async {
    await Future.wait([
      _storage.write(key: 'access_token', value: accessToken),
      _storage.write(key: 'refresh_token', value: refreshToken),
    ]);
  }

  static Future<void> clearTokens() async {
    await _storage.deleteAll();
  }
}

// Encrypted SQLite database
class EncryptedDatabase {
  static Future<Database> open() async {
    // Encryption key stored in secure storage, not hardcoded
    final secureStorage = const FlutterSecureStorage();
    String? dbKey = await secureStorage.read(key: 'db_encryption_key');

    if (dbKey == null) {
      // Generate and store key on first launch
      dbKey = _generateSecureKey();
      await secureStorage.write(key: 'db_encryption_key', value: dbKey);
    }

    final dbPath = p.join(await getDatabasesPath(), 'secure_app.db');

    return openDatabase(
      dbPath,
      password: dbKey, // SQLCipher encryption
      version: 1,
      onCreate: (db, version) async {
        await db.execute('''
          CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
          )
        ''');
        // Credit cards and sensitive data should NOT be stored locally
      },
    );
  }
}

// Secure temporary files
class SecureTempFiles {
  static Future<File> createSecureTemp(String prefix) async {
    final dir = await getTemporaryDirectory();
    final file = File(p.join(dir.path, '${prefix}_${DateTime.now().millisecondsSinceEpoch}'));
    return file;
  }

  static Future<void> secureDelete(File file) async {
    if (await file.exists()) {
      // Overwrite before deleting
      final length = await file.length();
      await file.writeAsBytes(List.filled(length, 0));
      await file.delete();
    }
  }
}
```

### Platform-Specific Notes

| Platform | Storage | Notes |
|----------|---------|-------|
| Android  | `EncryptedSharedPreferences` | Backed by Android Keystore. Requires API 23+. |
| Android  | `SharedPreferences` | XML file in `/data/data/<pkg>/shared_prefs/` — readable on rooted devices. |
| iOS      | Keychain | Set `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` for max security. |
| iOS      | `UserDefaults` | Plist file — not encrypted, not for secrets. |

### Mitigation Checklist

- [ ] `flutter_secure_storage` for all tokens, keys, and credentials
- [ ] SQLite databases encrypted with SQLCipher if they contain sensitive data
- [ ] No PII or secrets in `SharedPreferences` / `UserDefaults`
- [ ] Temporary files overwritten and deleted after use
- [ ] `android:allowBackup="false"` prevents backup extraction
- [ ] App snapshot/thumbnail blurred for sensitive screens
- [ ] Cache directories cleaned periodically

---

## M10: Insufficient Cryptography

**Description**: Use of weak, deprecated, or improperly implemented cryptographic
algorithms, hardcoded keys, or insufficient key management.

### Flutter-Specific Risk Scenarios

- Using MD5 or SHA1 for password hashing
- Hardcoded encryption keys in Dart source
- ECB mode for block ciphers
- Using `dart:math` `Random()` instead of `Random.secure()` for cryptographic purposes
- Implementing custom cryptography instead of proven libraries
- IV/nonce reuse in AES encryption

### WRONG (Insecure)

```dart
import 'dart:convert';
import 'dart:math';
import 'package:crypto/crypto.dart';

// MD5 for password hashing — broken
String hashPassword(String password) {
  return md5.convert(utf8.encode(password)).toString();
}

// Hardcoded key + ECB mode (implicit in some libraries)
String encryptData(String plaintext) {
  const key = 'MySuperSecretKey1234567890123456'; // Hardcoded!
  // Using a library that defaults to ECB mode — patterns preserved
  return AesEcb.encrypt(plaintext, key);
}

// Insecure random for tokens
String generateToken() {
  final random = Random(); // NOT cryptographically secure
  return List.generate(32, (_) => random.nextInt(256).toRadixString(16)).join();
}
```

### RIGHT (Secure)

```dart
import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';
import 'package:pointycastle/pointycastle.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

// Proper password hashing with Argon2 (via pointycastle or native plugin)
class PasswordHasher {
  /// Hash password with Argon2id — memory-hard, GPU-resistant
  static Future<String> hash(String password) async {
    // Use a native plugin like `hashlib` or `argon2_ffi` for Argon2id
    // Fallback: PBKDF2 with high iterations
    final salt = _secureRandomBytes(32);
    final pbkdf2 = KeyDerivator('SHA-256/HMAC/PBKDF2')
      ..init(Pbkdf2Parameters(salt, 600000, 32)); // 600k iterations

    final hash = pbkdf2.process(Uint8List.fromList(utf8.encode(password)));
    return '${base64.encode(salt)}:${base64.encode(hash)}';
  }

  static bool verify(String password, String stored) {
    final parts = stored.split(':');
    final salt = base64.decode(parts[0]);
    final expectedHash = base64.decode(parts[1]);

    final pbkdf2 = KeyDerivator('SHA-256/HMAC/PBKDF2')
      ..init(Pbkdf2Parameters(Uint8List.fromList(salt), 600000, 32));

    final actualHash = pbkdf2.process(Uint8List.fromList(utf8.encode(password)));
    return _constantTimeEquals(expectedHash, actualHash);
  }

  /// Constant-time comparison to prevent timing attacks
  static bool _constantTimeEquals(Uint8List a, Uint8List b) {
    if (a.length != b.length) return false;
    int result = 0;
    for (int i = 0; i < a.length; i++) {
      result |= a[i] ^ b[i];
    }
    return result == 0;
  }
}

// AES-256-GCM with proper key management and unique nonce
class SecureEncryption {
  static const _storage = FlutterSecureStorage();
  static const _keyAlias = 'app_encryption_key';

  /// Get or generate the encryption key stored in platform secure storage
  static Future<Uint8List> _getKey() async {
    String? keyBase64 = await _storage.read(key: _keyAlias);
    if (keyBase64 == null) {
      final key = _secureRandomBytes(32); // AES-256
      keyBase64 = base64.encode(key);
      await _storage.write(key: _keyAlias, value: keyBase64);
    }
    return base64.decode(keyBase64);
  }

  /// Encrypt with AES-256-GCM — authenticated encryption
  static Future<String> encrypt(String plaintext) async {
    final key = await _getKey();
    final nonce = _secureRandomBytes(12); // 96-bit nonce for GCM

    final cipher = GCMBlockCipher(AESEngine())
      ..init(true, AEADParameters(KeyParameter(key), 128, nonce, Uint8List(0)));

    final input = Uint8List.fromList(utf8.encode(plaintext));
    final output = cipher.process(input);

    // Prepend nonce to ciphertext for storage
    final combined = Uint8List(nonce.length + output.length);
    combined.setAll(0, nonce);
    combined.setAll(nonce.length, output);

    return base64.encode(combined);
  }

  /// Decrypt AES-256-GCM
  static Future<String> decrypt(String ciphertext) async {
    final key = await _getKey();
    final combined = base64.decode(ciphertext);

    final nonce = combined.sublist(0, 12);
    final encrypted = combined.sublist(12);

    final cipher = GCMBlockCipher(AESEngine())
      ..init(false, AEADParameters(KeyParameter(key), 128, nonce, Uint8List(0)));

    final decrypted = cipher.process(Uint8List.fromList(encrypted));
    return utf8.decode(decrypted);
  }
}

// Cryptographically secure random bytes
Uint8List _secureRandomBytes(int length) {
  final random = Random.secure();
  return Uint8List.fromList(List.generate(length, (_) => random.nextInt(256)));
}

// Secure token generation
String generateSecureToken({int length = 32}) {
  final bytes = _secureRandomBytes(length);
  return base64Url.encode(bytes);
}
```

### Platform-Specific Notes

| Platform | Notes |
|----------|-------|
| Android  | Android Keystore generates and stores keys in hardware (TEE/StrongBox). Use `flutter_secure_storage` which delegates to Keystore. |
| iOS      | Secure Enclave available for key generation. Keychain stores keys encrypted at rest. |

### Mitigation Checklist

- [ ] No MD5, SHA1, DES, RC4, or ECB mode in production
- [ ] Passwords hashed with Argon2id or PBKDF2 (600k+ iterations)
- [ ] AES-256-GCM (authenticated encryption) for data at rest
- [ ] Encryption keys in platform secure storage, never hardcoded
- [ ] `Random.secure()` for all security-sensitive random generation
- [ ] Unique IV/nonce for every encryption operation
- [ ] Constant-time comparison for hash/HMAC verification
- [ ] No custom cryptographic implementations — use proven libraries

---

## Common False Positives

When running security scanners on Flutter projects, the following often trigger alerts
that are **not** actual vulnerabilities. Do not waste time on these:

| Finding | Why It Is a False Positive |
|---------|---------------------------|
| `http://` in test files | Test fixtures and mock servers legitimately use HTTP. Only flag in `lib/`. |
| `android:usesCleartextTraffic="true"` in `debug` manifest | Flutter generates a debug manifest variant. Only flag in `main` or `release`. |
| `kDebugMode` checks in source | These are *guards*, not vulnerabilities. They gate debug-only behavior correctly. |
| `print()` in test files | Console output in tests is expected. Only flag `print()` in `lib/` for release builds. |
| Self-signed certs in `test/` or `integration_test/` | Test infrastructure commonly uses self-signed certificates. |
| `SharedPreferences` storing non-sensitive UI state | Theme preferences, onboarding flags, and locale settings are not secrets. |
| `Random()` in non-security contexts | Using `Random()` for UI animations, shuffling display lists, etc. is fine. |
| Base64 encoding flagged as "encryption" | Base64 is encoding, not encryption. Only flag if it is being *used as* encryption. |
| `allowBackup` in debug variant | Debug builds often allow backup for development convenience. Flag only in release. |

---

## Integration Points

### When to Run Security Checks

| Trigger | What to Run | Tools |
|---------|-------------|-------|
| **Pre-commit hook** | Hardcoded secrets scan, `print()` in `lib/` | `../scripts/secret-scanner.sh`, git-secrets |
| **PR review** | Full OWASP checklist review, dependency audit | `flutter pub outdated`, OSV Scanner, manual review against this document |
| **Nightly CI** | Full static analysis, dependency CVE check | `dart analyze`, `osv-scanner --lockfile=pubspec.lock`, `../scripts/owasp-audit.sh` |
| **Pre-release** | Binary protection verification, permission audit | Verify `--obfuscate` flag, review AndroidManifest permissions, ATS config, `../scripts/release-checklist.sh` |
| **Post-incident** | Targeted review of affected category | Re-scan specific M-category, update this reference if new pattern found |

### CI Pipeline Example

```yaml
# .github/workflows/security.yml
name: Security Audit
on:
  pull_request:
    branches: [main, develop]
  schedule:
    - cron: '0 3 * * 1'  # Weekly Monday 3am

jobs:
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2

      - name: Dependency audit
        run: |
          flutter pub get
          flutter pub outdated
          dart pub deps --style=compact

      - name: Static analysis
        run: dart analyze --fatal-infos

      - name: Secret scan
        run: bash scripts/secret-scanner.sh

      - name: OSV scan
        uses: google/osv-scanner-action/osv-scanner-action@v1
        with:
          scan-args: --lockfile=pubspec.lock
```

---

## Scanner Scripts Reference

All automated scanner scripts are located in `../scripts/`:

| Script | Purpose | Runs At |
|--------|---------|---------|
| `secret-scanner.sh` | Scans for hardcoded API keys, tokens, passwords in Dart source | Pre-commit, PR |
| `owasp-audit.sh` | Runs full M1-M10 automated checks against project structure | Nightly CI |
| `release-checklist.sh` | Verifies obfuscation, permissions, ATS, NSC, backup settings | Pre-release |
| `dependency-audit.sh` | Checks `pubspec.lock` against CVE databases | PR, Weekly |

> **Note**: These scripts complement but do not replace manual security review.
> Complex vulnerabilities in business logic (M3, M4) require human analysis.

---

*This reference is a living document. Update it when new Flutter-specific attack vectors
are discovered or when OWASP publishes revisions.*
