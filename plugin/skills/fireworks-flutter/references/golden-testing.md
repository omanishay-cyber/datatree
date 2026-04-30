# Golden Testing Reference (Alchemist + Flutter)

> Alchemist framework setup, CI vs platform test modes, GoldenTestGroup/GoldenTestScenario,
> font consistency, dark theme variant testing.

---

## 1. Alchemist Setup

```yaml
dev_dependencies:
  alchemist: ^0.10.0
  flutter_test:
    sdk: flutter
```

---

## 2. GoldenTestGroup and GoldenTestScenario

```dart
import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  goldenTest(
    'ProductCard renders correctly in all states',
    fileName: 'product_card',
    builder: () => GoldenTestGroup(
      scenarioConstraints: const BoxConstraints(maxWidth: 400),
      children: [
        GoldenTestScenario(
          name: 'default',
          child: ProductCard(
            product: Product(id: '1', name: 'Widget', price: 9.99, inStock: true),
          ),
        ),
        GoldenTestScenario(
          name: 'out of stock',
          child: ProductCard(
            product: Product(id: '2', name: 'Gadget', price: 19.99, inStock: false),
          ),
        ),
        GoldenTestScenario(
          name: 'long name',
          child: ProductCard(
            product: Product(
              id: '3',
              name: 'Super Ultra Premium Deluxe Widget Pro Max',
              price: 99.99,
              inStock: true,
            ),
          ),
        ),
        GoldenTestScenario(
          name: 'zero price',
          child: ProductCard(
            product: Product(id: '4', name: 'Free Item', price: 0, inStock: true),
          ),
        ),
      ],
    ),
  );
}
```

---

## 3. Dark Theme Variant Testing

```dart
void main() {
  // Light theme golden
  goldenTest(
    'LoginPage light theme',
    fileName: 'login_page_light',
    builder: () => GoldenTestGroup(
      children: [
        GoldenTestScenario(
          name: 'initial state',
          child: Theme(
            data: ThemeData.light(useMaterial3: true),
            child: const LoginPage(),
          ),
        ),
      ],
    ),
  );

  // Dark theme golden
  goldenTest(
    'LoginPage dark theme',
    fileName: 'login_page_dark',
    builder: () => GoldenTestGroup(
      children: [
        GoldenTestScenario(
          name: 'initial state',
          child: Theme(
            data: ThemeData.dark(useMaterial3: true),
            child: const LoginPage(),
          ),
        ),
      ],
    ),
  );

  // Both themes in one test
  goldenTest(
    'Button variants',
    fileName: 'button_variants',
    builder: () => GoldenTestGroup(
      children: [
        for (final brightness in Brightness.values)
          GoldenTestScenario(
            name: 'primary ${brightness.name}',
            child: Theme(
              data: ThemeData(brightness: brightness, useMaterial3: true),
              child: const PrimaryButton(label: 'Submit'),
            ),
          ),
      ],
    ),
  );
}
```

---

## 4. CI vs Platform Test Modes

### CI Mode (Recommended for CI pipelines)

Uses the Ahem font for consistent rendering across all platforms.

```dart
// test/flutter_test_config.dart
import 'dart:async';
import 'package:alchemist/alchemist.dart';

Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  // Use CI mode: renders with Ahem font, platform-independent
  return AlchemistConfig.runWithConfig(
    config: const AlchemistConfig(
      theme: ThemeData(fontFamily: 'Ahem'),
      platformGoldensConfig: PlatformGoldensConfig(enabled: false),
      ciGoldensConfig: CiGoldensConfig(enabled: true),
    ),
    run: testMain,
  );
}
```

### Platform Mode (For local development)

Uses real fonts, generates platform-specific goldens.

```dart
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  return AlchemistConfig.runWithConfig(
    config: AlchemistConfig(
      theme: ThemeData(useMaterial3: true),
      platformGoldensConfig: const PlatformGoldensConfig(enabled: true),
      ciGoldensConfig: const CiGoldensConfig(enabled: false),
    ),
    run: testMain,
  );
}
```

### Font Consistency (Ahem for CI)

The Ahem font renders every glyph as a square box, ensuring:
- Identical rendering across macOS, Linux, Windows
- No font-related golden test failures in CI
- Predictable text metrics

```dart
// For local development with real fonts, load them in test config:
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  TestWidgetsFlutterBinding.ensureInitialized();
  // Load custom fonts for accurate local goldens
  final fontData = rootBundle.load('assets/fonts/Inter-Regular.ttf');
  final fontLoader = FontLoader('Inter')..addFont(fontData);
  await fontLoader.load();
  return testMain();
}
```

---

## 5. Updating Goldens

```bash
# Update all golden files
flutter test --update-goldens

# Update specific test file goldens
flutter test --update-goldens test/golden/product_card_test.dart

# Run golden tests (compare against existing)
flutter test --tags=golden

# CI: fail if goldens don't match
flutter test --tags=golden --no-update-goldens
```

---

## 6. Golden Test Organization

```
test/
  golden/
    goldens/           # Generated golden images (git-tracked)
      ci/              # CI goldens (Ahem font)
        product_card.png
        login_page_light.png
        login_page_dark.png
      macos/           # Platform goldens (macOS)
      linux/           # Platform goldens (Linux)
    product_card_golden_test.dart
    login_page_golden_test.dart
    button_variants_golden_test.dart
```

---

## 7. Golden Testing Best Practices

| Practice | Why |
|---|---|
| Use CI goldens in pipelines | Platform goldens differ between macOS/Linux/Windows |
| Test both light and dark themes | Catches theme-specific rendering issues |
| Set explicit constraints | Prevents flaky tests from layout differences |
| Use Ahem font in CI | Eliminates cross-platform font rendering differences |
| Group related scenarios | One golden file per component, multiple scenarios |
| Track golden files in git | Team sees visual diffs in PRs |
| Review golden diffs carefully | A changed golden may indicate a regression |
