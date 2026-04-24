# Testing Flutter Applications Reference

> Comprehensive testing patterns covering unit tests, widget tests, integration tests,
> mocking with mocktail, Riverpod/BLoC test patterns, navigation testing, golden tests,
> HTTP mocking, coverage, CI/CD, performance testing, and accessibility testing.

---

## 1. Unit Testing

### Basic Test Structure

```dart
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Calculator', () {
    late Calculator calculator;

    setUp(() {
      calculator = Calculator();
    });

    tearDown(() {
      // Clean up if needed
    });

    test('adds two numbers correctly', () {
      expect(calculator.add(2, 3), equals(5));
    });

    test('divides two numbers correctly', () {
      expect(calculator.divide(10, 2), equals(5.0));
    });

    test('throws ArgumentError when dividing by zero', () {
      expect(
        () => calculator.divide(10, 0),
        throwsA(isA<ArgumentError>()),
      );
    });

    test('throws ArgumentError with specific message', () {
      expect(
        () => calculator.divide(10, 0),
        throwsA(
          isA<ArgumentError>().having(
            (e) => e.message,
            'message',
            'Cannot divide by zero',
          ),
        ),
      );
    });
  });
}
```

### Common Matchers

```dart
// Equality
expect(result, equals(42));
expect(result, isNot(equals(0)));

// Type checks
expect(result, isA<String>());
expect(result, isA<User>().having((u) => u.name, 'name', 'John'));

// Collections
expect(list, isEmpty);
expect(list, isNotEmpty);
expect(list, hasLength(3));
expect(list, contains('item'));
expect(list, containsAll(['a', 'b']));
expect(list, orderedEquals(['a', 'b', 'c']));
expect(map, containsPair('key', 'value'));

// Numeric
expect(value, greaterThan(5));
expect(value, lessThanOrEqualTo(10));
expect(value, inInclusiveRange(1, 10));
expect(value, closeTo(3.14, 0.01));

// Strings
expect(str, startsWith('Hello'));
expect(str, endsWith('world'));
expect(str, contains('middle'));
expect(str, matches(RegExp(r'^\d{3}-\d{4}$')));

// Exceptions
expect(() => fn(), throwsException);
expect(() => fn(), throwsA(isA<FormatException>()));
expect(() => fn(), throwsStateError);

// Null
expect(result, isNull);
expect(result, isNotNull);
```

---

## 2. Widget Testing

### Basic Widget Test

```dart
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('Counter increments when button is tapped', (tester) async {
    // Build the widget
    await tester.pumpWidget(
      const MaterialApp(home: CounterPage()),
    );

    // Verify initial state
    expect(find.text('0'), findsOneWidget);
    expect(find.text('1'), findsNothing);

    // Tap the increment button
    await tester.tap(find.byIcon(Icons.add));

    // Rebuild the widget after state change
    await tester.pump();

    // Verify the counter incremented
    expect(find.text('0'), findsNothing);
    expect(find.text('1'), findsOneWidget);
  });
}
```

### Finder Methods

```dart
// By text
find.text('Hello')                           // Exact text match
find.textContaining('Hel')                   // Partial match
find.widgetWithText(ElevatedButton, 'Submit') // Text within specific widget type

// By type
find.byType(ElevatedButton)
find.byType(CircularProgressIndicator)

// By key
find.byKey(const Key('submit_button'))
find.byKey(const ValueKey('product_1'))

// By icon
find.byIcon(Icons.add)
find.byIcon(Icons.delete)

// By widget predicate
find.byWidgetPredicate(
  (widget) => widget is Text && widget.data!.startsWith('Error'),
)

// Descendant / Ancestor
find.descendant(
  of: find.byType(ListTile),
  matching: find.text('John'),
)

find.ancestor(
  of: find.text('Submit'),
  matching: find.byType(Form),
)

// Matchers
expect(find.text('Hello'), findsOneWidget);
expect(find.text('Missing'), findsNothing);
expect(find.byType(ListTile), findsNWidgets(3));
expect(find.byType(ListTile), findsAtLeastNWidgets(1));
expect(find.byType(Card), findsAny);
```

### WidgetTester Actions

```dart
// Tap
await tester.tap(find.byType(ElevatedButton));

// Long press
await tester.longPress(find.byType(ListTile));

// Enter text
await tester.enterText(find.byType(TextField), 'hello@test.com');

// Drag / Scroll
await tester.drag(find.byType(ListView), const Offset(0, -300));
await tester.fling(find.byType(ListView), const Offset(0, -500), 1000);

// Rebuild
await tester.pump();                        // One frame
await tester.pump(const Duration(seconds: 1)); // After duration
await tester.pumpAndSettle();               // Until all animations complete

// Swipe to dismiss
await tester.drag(
  find.byKey(const ValueKey('item_1')),
  const Offset(500, 0),
);
await tester.pumpAndSettle();
```

### Testing with Providers

```dart
testWidgets('shows user name after loading', (tester) async {
  await tester.pumpWidget(
    MaterialApp(
      home: BlocProvider(
        create: (_) => UserCubit(MockUserRepository())..loadUser('1'),
        child: const UserProfilePage(),
      ),
    ),
  );

  // Wait for async operations
  await tester.pumpAndSettle();

  expect(find.text('John Doe'), findsOneWidget);
});
```

---

## 3. Integration Testing

### Setup

```yaml
# pubspec.yaml
dev_dependencies:
  integration_test:
    sdk: flutter
  flutter_test:
    sdk: flutter
```

```dart
// integration_test/app_test.dart
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:my_app/main.dart' as app;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('full login flow', (tester) async {
    app.main();
    await tester.pumpAndSettle();

    // Enter credentials
    await tester.enterText(
      find.byKey(const Key('email_field')),
      'test@example.com',
    );
    await tester.enterText(
      find.byKey(const Key('password_field')),
      'password123',
    );

    // Submit
    await tester.tap(find.byKey(const Key('login_button')));
    await tester.pumpAndSettle();

    // Verify navigation to home
    expect(find.text('Welcome'), findsOneWidget);
  });
}
```

Run integration tests:
```bash
flutter test integration_test/app_test.dart
# On device:
flutter test integration_test --device-id=<device_id>
```

### Patrol (Advanced Integration Testing)

```yaml
# pubspec.yaml
dev_dependencies:
  patrol: ^3.0.0
```

```dart
import 'package:patrol/patrol.dart';

void main() {
  patrolTest('login and browse products', ($) async {
    await $.pumpWidgetAndSettle(const MyApp());

    // Patrol uses $ shorthand for common operations
    await $(#email_field).enterText('test@example.com');
    await $(#password_field).enterText('password123');
    await $(#login_button).tap();

    // Wait for specific text to appear
    await $.pumpAndSettle();
    expect($('Welcome'), findsOneWidget);

    // Native interactions (notifications, permissions)
    await $.native.grantPermissionWhenInUse();
  });
}
```

---

## 4. Mocking with mocktail (Preferred -- No Codegen)

```yaml
# pubspec.yaml
dev_dependencies:
  mocktail: ^1.0.0
```

```dart
import 'package:mocktail/mocktail.dart';

// Create mock
class MockAuthRepository extends Mock implements AuthRepository {}
class MockDio extends Mock implements Dio {}

void main() {
  late MockAuthRepository mockRepo;

  setUp(() {
    mockRepo = MockAuthRepository();
  });

  // Register fallback values for custom types
  setUpAll(() {
    registerFallbackValue(LoginParams(email: '', password: ''));
    registerFallbackValue(User(id: '', name: '', email: '', role: UserRole.staff));
  });

  test('login returns user on success', () async {
    // Arrange
    final tUser = User(id: '1', name: 'Test', email: 't@t.com', role: UserRole.staff);

    when(() => mockRepo.login(
      email: any(named: 'email'),
      password: any(named: 'password'),
    )).thenAnswer((_) async => Right(tUser));

    // Act
    final result = await mockRepo.login(email: 'a@b.com', password: '123');

    // Assert
    expect(result, Right(tUser));

    // Verify calls
    verify(() => mockRepo.login(
      email: 'a@b.com',
      password: '123',
    )).called(1);

    verifyNever(() => mockRepo.logout());
    verifyNoMoreInteractions(mockRepo);
  });

  // Throwing exceptions
  test('login throws on server error', () async {
    when(() => mockRepo.login(
      email: any(named: 'email'),
      password: any(named: 'password'),
    )).thenThrow(ServerException(message: 'Error', statusCode: 500));

    expect(
      () => mockRepo.login(email: 'a@b.com', password: '123'),
      throwsA(isA<ServerException>()),
    );
  });

  // Sequential returns
  test('retry succeeds on second attempt', () async {
    var callCount = 0;
    when(() => mockRepo.login(
      email: any(named: 'email'),
      password: any(named: 'password'),
    )).thenAnswer((_) async {
      callCount++;
      if (callCount == 1) return const Left(NetworkFailure('timeout'));
      return Right(User(id: '1', name: 'Test', email: 't@t.com', role: UserRole.staff));
    });

    // First call fails
    final r1 = await mockRepo.login(email: 'a@b.com', password: '123');
    expect(r1.isLeft(), true);

    // Second call succeeds
    final r2 = await mockRepo.login(email: 'a@b.com', password: '123');
    expect(r2.isRight(), true);
  });
}
```

---

## 5. Testing with Riverpod

### Unit Testing Providers

```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('counterProvider starts at 0 and increments', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);

    expect(container.read(counterProvider), 0);

    container.read(counterProvider.notifier).increment();
    expect(container.read(counterProvider), 1);
  });

  test('userProvider fetches user from repository', () async {
    final mockRepo = MockUserRepository();
    when(() => mockRepo.getUser('1')).thenAnswer(
      (_) async => User(id: '1', name: 'John'),
    );

    final container = ProviderContainer(
      overrides: [
        userRepositoryProvider.overrideWithValue(mockRepo),
      ],
    );
    addTearDown(container.dispose);

    // Listen to AsyncValue changes
    final listener = Listener<AsyncValue<User>>();
    container.listen(userProvider('1'), listener.call, fireImmediately: true);

    // Wait for async completion
    await container.read(userProvider('1').future);

    verify(() => listener(
      const AsyncValue.loading(),
      AsyncValue.data(User(id: '1', name: 'John')),
    )).called(1);
  });
}

// Listener helper for verify
class Listener<T> extends Mock {
  void call(T? previous, T next);
}
```

### Widget Testing with Riverpod Overrides

```dart
testWidgets('shows user profile', (tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        userProvider.overrideWith(
          (ref) => AsyncValue.data(User(id: '1', name: 'John')),
        ),
      ],
      child: const MaterialApp(home: UserProfilePage()),
    ),
  );

  await tester.pumpAndSettle();
  expect(find.text('John'), findsOneWidget);
});

testWidgets('shows loading state', (tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        userProvider.overrideWith((ref) => const AsyncValue.loading()),
      ],
      child: const MaterialApp(home: UserProfilePage()),
    ),
  );

  expect(find.byType(CircularProgressIndicator), findsOneWidget);
});
```

---

## 6. Testing with BLoC

```dart
import 'package:bloc_test/bloc_test.dart';

void main() {
  group('AuthBloc', () {
    late MockAuthRepository mockRepo;

    setUp(() {
      mockRepo = MockAuthRepository();
    });

    blocTest<AuthBloc, AuthState>(
      'emits [AuthLoading, AuthSuccess] on successful login',
      build: () {
        when(() => mockRepo.login(
          email: any(named: 'email'),
          password: any(named: 'password'),
        )).thenAnswer((_) async => Right(tUser));
        return AuthBloc(mockRepo);
      },
      act: (bloc) => bloc.add(LoginRequested(email: 't@t.com', password: 'pw')),
      expect: () => [
        isA<AuthLoading>(),
        isA<AuthSuccess>(),
      ],
    );

    // Test initial state
    blocTest<AuthBloc, AuthState>(
      'has AuthInitial as initial state',
      build: () => AuthBloc(mockRepo),
      verify: (bloc) => expect(bloc.state, isA<AuthInitial>()),
    );

    // Test with seed state
    blocTest<AuthBloc, AuthState>(
      'emits [AuthInitial] on logout from authenticated state',
      build: () {
        when(() => mockRepo.logout()).thenAnswer((_) async {});
        return AuthBloc(mockRepo);
      },
      seed: () => AuthSuccess(tUser),
      act: (bloc) => bloc.add(LogoutRequested()),
      expect: () => [isA<AuthInitial>()],
    );

    // Test with wait (for debounced events)
    blocTest<SearchBloc, SearchState>(
      'debounces search input',
      build: () {
        when(() => mockRepo.search(any()))
            .thenAnswer((_) async => ['result']);
        return SearchBloc(mockRepo);
      },
      act: (bloc) {
        bloc.add(SearchQueryChanged('a'));
        bloc.add(SearchQueryChanged('ab'));
        bloc.add(SearchQueryChanged('abc'));
      },
      wait: const Duration(milliseconds: 350),
      expect: () => [
        isA<SearchLoading>(),
        isA<SearchLoaded>(),
      ],
      verify: (_) {
        verify(() => mockRepo.search('abc')).called(1);
        verifyNever(() => mockRepo.search('a'));
        verifyNever(() => mockRepo.search('ab'));
      },
    );
  });
}
```

---

## 7. Testing Navigation

### GoRouter Testing

```dart
testWidgets('navigates to profile on tap', (tester) async {
  final router = GoRouter(
    initialLocation: '/',
    routes: [
      GoRoute(path: '/', builder: (_, __) => const HomePage()),
      GoRoute(path: '/profile', builder: (_, __) => const ProfilePage()),
    ],
  );

  await tester.pumpWidget(
    MaterialApp.router(routerConfig: router),
  );

  await tester.tap(find.byKey(const Key('profile_button')));
  await tester.pumpAndSettle();

  expect(find.byType(ProfilePage), findsOneWidget);
});
```

### NavigatorObserver for Testing

```dart
class MockNavigatorObserver extends Mock implements NavigatorObserver {}

testWidgets('navigates when login succeeds', (tester) async {
  final observer = MockNavigatorObserver();

  await tester.pumpWidget(
    MaterialApp(
      home: const LoginPage(),
      navigatorObservers: [observer],
      routes: {
        '/home': (_) => const HomePage(),
      },
    ),
  );

  // Trigger navigation...
  await tester.pumpAndSettle();

  verify(() => observer.didPush(any(), any())).called(greaterThan(0));
});
```

---

## 8. Testing Async Code

```dart
test('stream emits values in order', () async {
  final stream = counterStream();

  await expectLater(
    stream,
    emitsInOrder([0, 1, 2, 3]),
  );
});

test('future completes with value', () async {
  await expectLater(
    fetchUser('1'),
    completion(isA<User>().having((u) => u.name, 'name', 'John')),
  );
});

test('stream emits error', () async {
  await expectLater(
    failingStream(),
    emitsError(isA<NetworkException>()),
  );
});

test('stream emits values then done', () async {
  await expectLater(
    finiteStream(),
    emitsInOrder([1, 2, 3, emitsDone]),
  );
});
```

---

## 9. Golden Testing

```dart
testWidgets('product card renders correctly', (tester) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: ProductCard(
          product: Product(id: '1', name: 'Widget', price: 9.99),
        ),
      ),
    ),
  );

  await expectLater(
    find.byType(ProductCard),
    matchesGoldenFile('goldens/product_card.png'),
  );
});
```

Generate/update golden files:
```bash
flutter test --update-goldens

# Platform-specific goldens
flutter test --update-goldens --tags=golden
```

### Handling Font Differences Across Platforms

```dart
// test/flutter_test_config.dart
import 'dart:async';
import 'package:flutter_test/flutter_test.dart';

Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  // Load fonts for golden tests
  await loadAppFonts();
  return testMain();
}
```

### Screenshot-Based Golden Testing

```dart
// For full-page goldens
testWidgets('login page golden -- light theme', (tester) async {
  await tester.binding.setSurfaceSize(const Size(375, 812)); // iPhone size

  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData.light(useMaterial3: true),
      home: const LoginPage(),
    ),
  );

  await tester.pumpAndSettle();

  await expectLater(
    find.byType(MaterialApp),
    matchesGoldenFile('goldens/login_page_light.png'),
  );
});

testWidgets('login page golden -- dark theme', (tester) async {
  await tester.binding.setSurfaceSize(const Size(375, 812));

  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData.dark(useMaterial3: true),
      home: const LoginPage(),
    ),
  );

  await tester.pumpAndSettle();

  await expectLater(
    find.byType(MaterialApp),
    matchesGoldenFile('goldens/login_page_dark.png'),
  );
});
```

---

## 10. HTTP Testing

### MockClient

```dart
import 'package:http/testing.dart';

test('fetches products from API', () async {
  final mockClient = MockClient((request) async {
    if (request.url.path == '/api/products') {
      return http.Response(
        json.encode([{'id': '1', 'name': 'Widget', 'price': 9.99}]),
        200,
        headers: {'content-type': 'application/json'},
      );
    }
    return http.Response('Not Found', 404);
  });

  final repo = ProductRepository(client: mockClient);
  final products = await repo.getAll();

  expect(products, hasLength(1));
  expect(products.first.name, 'Widget');
});
```

### Dio Interceptors for Testing

```dart
class MockInterceptor extends Interceptor {
  @override
  void onRequest(RequestOptions options, RequestInterceptorHandler handler) {
    // Return mock response instead of making real request
    if (options.path.contains('/products')) {
      handler.resolve(Response(
        requestOptions: options,
        statusCode: 200,
        data: [{'id': '1', 'name': 'Widget'}],
      ));
    } else {
      handler.next(options);
    }
  }
}

test('uses mock interceptor', () async {
  final dio = Dio()..interceptors.add(MockInterceptor());
  final repo = ProductRepository(dio: dio);
  final products = await repo.getAll();
  expect(products, isNotEmpty);
});
```

---

## 11. Database Testing

### In-Memory Database

```dart
test('saves and retrieves products from database', () async {
  // Use in-memory database for testing
  final db = await openDatabase(
    inMemoryDatabasePath,
    version: 1,
    onCreate: (db, version) async {
      await db.execute('''
        CREATE TABLE products (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          price REAL NOT NULL
        )
      ''');
    },
  );

  final datasource = ProductLocalDataSource(db);

  // Insert
  await datasource.cacheProducts([
    ProductModel(id: '1', name: 'Widget', price: 9.99),
  ]);

  // Retrieve
  final products = await datasource.getCachedProducts();
  expect(products, hasLength(1));
  expect(products.first.name, 'Widget');

  await db.close();
});
```

### Drift (Moor) In-Memory Testing

```dart
test('drift database operations', () async {
  final db = AppDatabase(NativeDatabase.memory());
  addTearDown(db.close);

  await db.into(db.products).insert(ProductsCompanion.insert(
    name: 'Widget',
    price: 9.99,
  ));

  final products = await db.select(db.products).get();
  expect(products, hasLength(1));
});
```

---

## 12. Test Coverage

```bash
# Generate coverage
flutter test --coverage

# Generate HTML report (requires lcov)
genhtml coverage/lcov.info -o coverage/html
open coverage/html/index.html

# Exclude generated files from coverage
# coverage/lcov.info filtering:
lcov --remove coverage/lcov.info \
  '*.g.dart' \
  '*.freezed.dart' \
  '*/generated/*' \
  -o coverage/filtered_lcov.info
```

### Coverage in analysis_options.yaml

```yaml
# analysis_options.yaml
analyzer:
  exclude:
    - "**/*.g.dart"
    - "**/*.freezed.dart"
    - "**/generated/**"
```

---

## 13. CI/CD Testing with GitHub Actions

```yaml
# .github/workflows/flutter-ci.yml
name: Flutter CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: subosito/flutter-action@v2
        with:
          flutter-version: '3.24.0'
          channel: 'stable'

      - name: Install dependencies
        run: flutter pub get

      - name: Analyze
        run: flutter analyze

      - name: Run tests
        run: flutter test --coverage

      - name: Check coverage
        uses: VeryGoodOpenSource/very_good_coverage@v3
        with:
          min_coverage: 80

  test-matrix:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
          - os: macos-latest
            platform: ios
          - os: ubuntu-latest
            platform: android
          - os: ubuntu-latest
            platform: web
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2
        with:
          flutter-version: '3.24.0'

      - run: flutter pub get
      - run: flutter test

      - name: Build
        run: flutter build ${{ matrix.platform }} --release
        if: matrix.platform != 'ios'

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: build-${{ matrix.platform }}
          path: build/
```

---

## 14. Performance Testing

```dart
// flutter_driver based (legacy but stable)
import 'package:flutter_driver/flutter_driver.dart';
import 'package:test/test.dart';

void main() {
  late FlutterDriver driver;

  setUpAll(() async {
    driver = await FlutterDriver.connect();
  });

  tearDownAll(() async {
    await driver.close();
  });

  test('scrolling performance', () async {
    final timeline = await driver.traceAction(() async {
      await driver.scroll(
        find.byValueKey('product_list'),
        0, -3000,
        const Duration(seconds: 3),
      );
    });

    final summary = TimelineSummary.summarize(timeline);
    await summary.writeTimelineToFile('scroll_perf', pretty: true);

    // Assert frame build times
    expect(summary.computeMissedFrameBudgetCount(), lessThan(5));
  });
}
```

### Benchmark Widgets

```dart
// test/benchmarks/widget_benchmark.dart
import 'package:flutter_test/flutter_test.dart';

void main() {
  benchmarkWidgets('ProductList build performance', (tester) async {
    final products = List.generate(
      1000,
      (i) => Product(id: '$i', name: 'P$i', price: i * 1.0),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ProductList(products: products),
        ),
      ),
    );
  });
}
```

### Integration Test with Timeline

```dart
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('measure scroll performance', (tester) async {
    final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();

    await tester.pumpWidget(const MyApp());
    await tester.pumpAndSettle();

    await binding.traceAction(() async {
      await tester.fling(
        find.byType(ListView),
        const Offset(0, -500),
        10000,
      );
      await tester.pumpAndSettle();

      await tester.fling(
        find.byType(ListView),
        const Offset(0, 500),
        10000,
      );
      await tester.pumpAndSettle();
    }, reportKey: 'scroll_timeline');
  });
}
```

---

## 15. Accessibility Testing

### Semantic Tree Assertions

```dart
testWidgets('product card has correct semantics', (tester) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: ProductCard(
          product: Product(id: '1', name: 'Widget', price: 9.99),
        ),
      ),
    ),
  );

  // Get semantics for a specific widget
  final semantics = tester.getSemantics(find.byType(ProductCard));
  expect(semantics.label, contains('Widget'));
  expect(semantics.label, contains('9.99'));
});

testWidgets('buttons have accessible labels', (tester) async {
  await tester.pumpWidget(
    const MaterialApp(home: Scaffold(body: AddToCartButton())),
  );

  final semantics = tester.getSemantics(find.byType(IconButton));
  expect(semantics.label, isNotEmpty);
  expect(semantics.hasAction(SemanticsAction.tap), isTrue);
});
```

### Guideline Checks

```dart
testWidgets('meets accessibility guidelines', (tester) async {
  final handle = tester.ensureSemantics();

  await tester.pumpWidget(const MaterialApp(home: LoginPage()));

  // Check minimum tap target size (48x48)
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));

  // Check text contrast
  await expectLater(tester, meetsGuideline(textContrastGuideline));

  // Check labeled tap targets
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));

  handle.dispose();
});
```

### SemanticsDebugger for Visual Debugging

```dart
// Wrap your app to see semantic tree visually during development
SemanticsDebugger(
  child: const MyApp(),
)
```

---

## 16. Test Organization Best Practices

```
test/
├── unit/
│   ├── core/
│   │   └── utils/
│   │       └── input_converter_test.dart
│   └── features/
│       ├── auth/
│       │   ├── domain/
│       │   │   └── usecases/
│       │   │       └── login_test.dart
│       │   └── data/
│       │       └── repositories/
│       │           └── auth_repository_impl_test.dart
│       └── products/
├── widget/
│   └── features/
│       ├── auth/
│       │   └── presentation/
│       │       ├── pages/
│       │       │   └── login_page_test.dart
│       │       └── widgets/
│       │           └── login_form_test.dart
│       └── products/
├── golden/
│   └── goldens/                    # Generated golden files
│       ├── product_card.png
│       └── login_page.png
├── fixtures/                       # Test data
│   ├── user.json
│   └── products.json
├── helpers/                        # Shared test utilities
│   ├── pump_app.dart              # Common pumpWidget wrapper
│   ├── mock_providers.dart
│   └── test_data.dart
└── integration_test/
    └── app_test.dart

# Running specific test suites
flutter test test/unit/
flutter test test/widget/
flutter test --tags=golden
flutter test integration_test/
```

### Test Helpers

```dart
// test/helpers/pump_app.dart
extension PumpApp on WidgetTester {
  Future<void> pumpApp(Widget widget, {List<Override>? overrides}) async {
    await pumpWidget(
      ProviderScope(
        overrides: overrides ?? [],
        child: MaterialApp(
          home: Scaffold(body: widget),
        ),
      ),
    );
  }
}

// Usage in tests:
testWidgets('renders correctly', (tester) async {
  await tester.pumpApp(const ProductCard(product: tProduct));
  expect(find.text(tProduct.name), findsOneWidget);
});
```

### Test Data Fixtures

```dart
// test/helpers/test_data.dart
final tUser = User(id: '1', name: 'Test User', email: 't@t.com', role: UserRole.staff);
final tProduct = Product(id: '1', name: 'Test Widget', price: 9.99);
final tProducts = List.generate(
  10,
  (i) => Product(id: '$i', name: 'Product $i', price: i * 10.0),
);

// JSON fixture loading
Future<Map<String, dynamic>> loadFixture(String name) async {
  final file = File('test/fixtures/$name');
  return json.decode(await file.readAsString());
}
```
