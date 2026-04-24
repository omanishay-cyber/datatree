# fireworks-flutter — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 4. Riverpod Quick-Reference

```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';

// --- Provider Types ---

// Simple value (computed, no state change)
final greetingProvider = Provider<String>((ref) => 'Hello, Flutter!');

// Mutable state (simple types)
final counterProvider = StateProvider<int>((ref) => 0);

// Async data (one-shot)
final userProvider = FutureProvider<User>((ref) async {
  final repo = ref.watch(authRepositoryProvider);
  return repo.getCurrentUser();
});

// Async data (real-time stream)
final messagesProvider = StreamProvider<List<Message>>((ref) {
  final repo = ref.watch(chatRepositoryProvider);
  return repo.watchMessages();
});

// Complex state with methods (Notifier — replaces StateNotifier)
final cartProvider = NotifierProvider<CartNotifier, CartState>(CartNotifier.new);

class CartNotifier extends Notifier<CartState> {
  @override
  CartState build() => const CartState(items: []);

  void addItem(Product product) {
    state = state.copyWith(items: [...state.items, product]);
  }

  void removeItem(String productId) {
    state = state.copyWith(
      items: state.items.where((i) => i.id != productId).toList(),
    );
  }
}

// Complex async state (AsyncNotifier — replaces AsyncNotifier)
final todosProvider =
    AsyncNotifierProvider<TodosNotifier, List<Todo>>(TodosNotifier.new);

class TodosNotifier extends AsyncNotifier<List<Todo>> {
  @override
  Future<List<Todo>> build() async {
    final repo = ref.watch(todoRepositoryProvider);
    return repo.fetchAll();
  }

  Future<void> add(Todo todo) async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      final repo = ref.read(todoRepositoryProvider);
      await repo.add(todo);
      return repo.fetchAll();
    });
  }
}

// --- Reading Providers ---
// In widgets:
class MyWidget extends ConsumerWidget {
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final count = ref.watch(counterProvider);          // Rebuilds on change
    final user = ref.watch(userProvider);              // AsyncValue<User>

    return user.when(
      data: (user) => Text(user.name),
      loading: () => const CircularProgressIndicator(),
      error: (err, stack) => Text('Error: $err'),
    );
  }
}

// In callbacks (never use ref.watch in callbacks!):
onPressed: () => ref.read(counterProvider.notifier).state++,

// Side effects:
ref.listen(authProvider, (prev, next) {
  if (next is AuthError) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(next.message)));
  }
});
```

See `references/riverpod-patterns.md` for full patterns including code generation, families, testing, and pagination.

---

## 5. BLoC Quick-Reference

```dart
import 'package:flutter_bloc/flutter_bloc.dart';

// --- Events ---
sealed class AuthEvent {}
class LoginRequested extends AuthEvent {
  final String email;
  final String password;
  LoginRequested({required this.email, required this.password});
}
class LogoutRequested extends AuthEvent {}
class AuthCheckRequested extends AuthEvent {}

// --- States ---
sealed class AuthState {}
class AuthInitial extends AuthState {}
class AuthLoading extends AuthState {}
class AuthAuthenticated extends AuthState {
  final User user;
  AuthAuthenticated(this.user);
}
class AuthUnauthenticated extends AuthState {}
class AuthError extends AuthState {
  final String message;
  AuthError(this.message);
}

// --- Bloc ---
class AuthBloc extends Bloc<AuthEvent, AuthState> {
  final AuthRepository _authRepo;

  AuthBloc(this._authRepo) : super(AuthInitial()) {
    on<LoginRequested>(_onLogin);
    on<LogoutRequested>(_onLogout);
    on<AuthCheckRequested>(_onAuthCheck);
  }

  Future<void> _onLogin(LoginRequested event, Emitter<AuthState> emit) async {
    emit(AuthLoading());
    try {
      final user = await _authRepo.login(event.email, event.password);
      emit(AuthAuthenticated(user));
    } catch (e) {
      emit(AuthError(e.toString()));
    }
  }

  Future<void> _onLogout(LogoutRequested event, Emitter<AuthState> emit) async {
    await _authRepo.logout();
    emit(AuthUnauthenticated());
  }

  Future<void> _onAuthCheck(AuthCheckRequested event, Emitter<AuthState> emit) async {
    final user = await _authRepo.getCurrentUser();
    if (user != null) {
      emit(AuthAuthenticated(user));
    } else {
      emit(AuthUnauthenticated());
    }
  }
}
```

See `references/bloc-patterns.md` for full patterns including Cubit, BlocConsumer, multi-BLoC, hydration, and testing.

---

## 6. Navigation (GoRouter)

```dart
import 'package:go_router/go_router.dart';

final routerProvider = Provider<GoRouter>((ref) {
  final authState = ref.watch(authProvider);

  return GoRouter(
    initialLocation: '/',
    debugLogDiagnostics: true,
    redirect: (context, state) {
      final isLoggedIn = authState is AuthAuthenticated;
      final isLoginRoute = state.matchedLocation == '/login';

      if (!isLoggedIn && !isLoginRoute) return '/login';
      if (isLoggedIn && isLoginRoute) return '/';
      return null;
    },
    routes: [
      // Basic route
      GoRoute(
        path: '/',
        name: 'home',
        builder: (context, state) => const HomeScreen(),
      ),
      // Route with path parameter
      GoRoute(
        path: '/product/:id',
        name: 'product',
        builder: (context, state) {
          final id = state.pathParameters['id']!;
          return ProductScreen(productId: id);
        },
      ),
      // Route with query parameter
      GoRoute(
        path: '/search',
        name: 'search',
        builder: (context, state) {
          final query = state.uri.queryParameters['q'] ?? '';
          return SearchScreen(query: query);
        },
      ),
      // Nested navigation with ShellRoute
      ShellRoute(
        builder: (context, state, child) => ScaffoldWithNavBar(child: child),
        routes: [
          GoRoute(path: '/home', builder: (_, __) => const HomeTab()),
          GoRoute(path: '/orders', builder: (_, __) => const OrdersTab()),
          GoRoute(path: '/profile', builder: (_, __) => const ProfileTab()),
        ],
      ),
    ],
    errorBuilder: (context, state) => ErrorScreen(error: state.error),
  );
});

// Navigation in widgets:
context.go('/product/123');                     // Replace stack
context.push('/product/123');                   // Push onto stack
context.goNamed('product', pathParameters: {'id': '123'});  // Named route
context.pop();                                  // Go back
```

---

## 7. Widget Composition Rules

### Rule 1: Extract Widgets as Classes, Not Methods

```dart
// BAD - method extraction prevents rebuild optimization
Widget _buildHeader() => Container(...);

// GOOD - class extraction enables const and selective rebuilds
class HeaderWidget extends StatelessWidget {
  const HeaderWidget({super.key});

  @override
  Widget build(BuildContext context) => Container(...);
}
```

### Rule 2: Const Constructors Everywhere

```dart
// BAD
class MyWidget extends StatelessWidget {
  MyWidget({super.key}); // Missing const

  @override
  Widget build(BuildContext context) {
    return Padding(             // Missing const
      padding: EdgeInsets.all(8), // Missing const
      child: Text('Hello'),       // Missing const
    );
  }
}

// GOOD
class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.all(8),
      child: Text('Hello'),
    );
  }
}
```

### Rule 3: Keep build() Under 30 Lines

If build() exceeds 30 lines, extract sub-widgets into their own classes.

### Rule 4: Composition Over Inheritance

```dart
// BAD - extending widgets
class FancyButton extends ElevatedButton { ... }

// GOOD - composing widgets
class FancyButton extends StatelessWidget {
  final String label;
  final VoidCallback onPressed;
  const FancyButton({super.key, required this.label, required this.onPressed});

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: onPressed,
      style: ElevatedButton.styleFrom(/* custom styling */),
      child: Text(label),
    );
  }
}
```

---

## 8. Animation Quick-Reference

```dart
// --- Implicit Animations (simple, declarative) ---
AnimatedContainer(
  duration: const Duration(milliseconds: 300),
  curve: Curves.easeOutCubic,
  width: _expanded ? 200 : 100,
  height: _expanded ? 200 : 100,
  decoration: BoxDecoration(
    color: _expanded ? Colors.blue : Colors.red,
    borderRadius: BorderRadius.circular(_expanded ? 24 : 8),
  ),
)

AnimatedOpacity(
  opacity: _visible ? 1.0 : 0.0,
  duration: const Duration(milliseconds: 200),
  child: const Text('Fade me'),
)

AnimatedSlide(
  offset: _visible ? Offset.zero : const Offset(0, 1),
  duration: const Duration(milliseconds: 300),
  child: const Card(child: Text('Slide up')),
)

// --- Explicit Animations (full control) ---
class PulseAnimation extends StatefulWidget {
  const PulseAnimation({super.key});

  @override
  State<PulseAnimation> createState() => _PulseAnimationState();
}

class _PulseAnimationState extends State<PulseAnimation>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _scale;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(milliseconds: 1000),
      vsync: this,
    )..repeat(reverse: true);
    _scale = Tween<double>(begin: 1.0, end: 1.2).animate(
      CurvedAnimation(parent: _controller, curve: Curves.easeInOut),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ScaleTransition(scale: _scale, child: const Icon(Icons.favorite));
  }
}

// --- Hero Animations ---
// Source screen
Hero(tag: 'product-${product.id}', child: Image.network(product.imageUrl))
// Destination screen
Hero(tag: 'product-${product.id}', child: Image.network(product.imageUrl))

// --- Staggered Animations ---
// Use Interval to stagger multiple animations on one controller
final slideAnimation = Tween<Offset>(
  begin: const Offset(0, 0.5), end: Offset.zero,
).animate(CurvedAnimation(
  parent: _controller,
  curve: const Interval(0.0, 0.6, curve: Curves.easeOut),
));
final fadeAnimation = Tween<double>(begin: 0, end: 1).animate(
  CurvedAnimation(
    parent: _controller,
    curve: const Interval(0.2, 0.8, curve: Curves.easeIn),
  ),
);
```

---

## 9. Platform Channels (Flutter <-> Native)

```dart
// --- Method Channel (request/response) ---
import 'package:flutter/services.dart';

class NativeBridge {
  static const _channel = MethodChannel('com.example.app/native');

  static Future<int> getBatteryLevel() async {
    try {
      final int result = await _channel.invokeMethod('getBatteryLevel');
      return result;
    } on PlatformException catch (e) {
      throw Exception('Failed to get battery level: ${e.message}');
    }
  }

  static Future<String> encryptData(String data) async {
    final String result = await _channel.invokeMethod('encrypt', {'data': data});
    return result;
  }
}

// --- Event Channel (streaming data from native) ---
class SensorService {
  static const _eventChannel = EventChannel('com.example.app/accelerometer');

  static Stream<AccelerometerData> get accelerometerStream {
    return _eventChannel.receiveBroadcastStream().map((event) {
      final map = Map<String, double>.from(event as Map);
      return AccelerometerData(x: map['x']!, y: map['y']!, z: map['z']!);
    });
  }
}

// --- BasicMessageChannel (bidirectional messages) ---
const channel = BasicMessageChannel<String>('com.example.app/messages', StringCodec());
channel.setMessageHandler((message) async {
  // Handle message from native
  return 'Flutter received: $message';
});
```

---

## 10. Testing Strategy

```dart
// --- Widget Test ---
import 'package:flutter_test/flutter_test.dart';

testWidgets('Counter increments when FAB tapped', (WidgetTester tester) async {
  await tester.pumpWidget(const MaterialApp(home: CounterPage()));

  // Verify initial state
  expect(find.text('0'), findsOneWidget);
  expect(find.text('1'), findsNothing);

  // Tap the FAB
  await tester.tap(find.byIcon(Icons.add));
  await tester.pump(); // Trigger rebuild

  // Verify state changed
  expect(find.text('0'), findsNothing);
  expect(find.text('1'), findsOneWidget);
});

// --- Unit Test for Use Case ---
test('LoginUseCase returns user on success', () async {
  final mockRepo = MockAuthRepository();
  when(() => mockRepo.login('test@test.com', 'password'))
      .thenAnswer((_) async => User(id: '1', name: 'Test'));

  final useCase = LoginUseCase(mockRepo);
  final result = await useCase(fakeLoginFixture); // email + password fields are populated by the fixture

  expect(result, isA<User>());
  expect(result.name, 'Test');
  verify(() => mockRepo.login('test@test.com', 'password')).called(1);
});

// --- BLoC Test ---
blocTest<AuthBloc, AuthState>(
  'emits [AuthLoading, AuthAuthenticated] on successful login',
  build: () {
    when(() => mockRepo.login(any(), any()))
        .thenAnswer((_) async => User(id: '1', name: 'Test'));
    return AuthBloc(mockRepo);
  },
  act: (bloc) => bloc.add(LoginRequested(email: 'a@b.com', password: FIXTURE_VALUE /* redacted-for-docs */)),
  expect: () => [isA<AuthLoading>(), isA<AuthAuthenticated>()],
);

// --- Integration Test ---
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Full login flow', (tester) async {
    app.main();
    await tester.pumpAndSettle();

    // Enter credentials
    await tester.enterText(find.byKey(const Key('email_field')), 'test@test.com');
    await tester.enterText(find.byKey(const Key('password_field')), 'password123');
    await tester.tap(find.byKey(const Key('login_button')));
    await tester.pumpAndSettle();

    // Verify navigation to home
    expect(find.text('Welcome'), findsOneWidget);
  });
}
```

See `references/testing-patterns.md` for golden tests, mocking strategies, and CI setup.

---

## 11. Premium UI Patterns

### Material 3 Theming

```dart
final lightTheme = ThemeData(
  useMaterial3: true,
  colorScheme: ColorScheme.fromSeed(
    seedColor: const Color(0xFF6750A4),
    brightness: Brightness.light,
  ),
  textTheme: GoogleFonts.interTextTheme(),
  cardTheme: CardTheme(
    elevation: 0,
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
    color: ColorScheme.fromSeed(seedColor: const Color(0xFF6750A4)).surfaceContainerLow,
  ),
);

final darkTheme = ThemeData(
  useMaterial3: true,
  colorScheme: ColorScheme.fromSeed(
    seedColor: const Color(0xFF6750A4),
    brightness: Brightness.dark,
  ),
  textTheme: GoogleFonts.interTextTheme(ThemeData.dark().textTheme),
);
```

### Custom Theme Extensions

```dart
class BrandColors extends ThemeExtension<BrandColors> {
  final Color success;
  final Color warning;
  final Color info;

  const BrandColors({required this.success, required this.warning, required this.info});

  @override
  BrandColors copyWith({Color? success, Color? warning, Color? info}) {
    return BrandColors(
      success: success ?? this.success,
      warning: warning ?? this.warning,
      info: info ?? this.info,
    );
  }

  @override
  BrandColors lerp(BrandColors? other, double t) {
    return BrandColors(
      success: Color.lerp(success, other?.success, t)!,
      warning: Color.lerp(warning, other?.warning, t)!,
      info: Color.lerp(info, other?.info, t)!,
    );
  }
}

// Usage:
final brandColors = Theme.of(context).extension<BrandColors>()!;
```

### Responsive Layout

```dart
class ResponsiveLayout extends StatelessWidget {
  final Widget mobile;
  final Widget? tablet;
  final Widget desktop;

  const ResponsiveLayout({
    super.key,
    required this.mobile,
    this.tablet,
    required this.desktop,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth >= 1200) return desktop;
        if (constraints.maxWidth >= 600) return tablet ?? desktop;
        return mobile;
      },
    );
  }
}
```

See `references/widget-catalog.md` for complete premium widget patterns.

---

## 12. Performance Optimization

| Problem | Solution |
|---------|---------|
| Unnecessary rebuilds | Use `const`, extract widgets, use `select` with Riverpod |
| Slow list scrolling | `ListView.builder` with `itemExtent` or `prototypeItem` |
| Large images | `cached_network_image`, resize on server, use `cacheWidth`/`cacheHeight` |
| Janky animations | `RepaintBoundary`, run animations on GPU with `Transform` |
| Slow app startup | Deferred loading with `deferred as`, lazy route loading |
| Memory leaks | Dispose controllers, cancel streams, use `autoDispose` providers |
| Heavy computation | `compute()` or `Isolate.run()` for CPU-intensive work |

### Profiling Checklist

```bash
flutter run --profile                 # Profile mode (not debug!)
flutter run --release                 # Test release performance
flutter pub run devtools              # Open DevTools
```

- **Widget Rebuild Tracker**: Enable in DevTools > Performance > Track Widget Rebuilds
- **Timeline**: Check for frames exceeding 16ms (60fps target)
- **Memory**: Watch for monotonically increasing memory (leak indicator)

---

## 13. Dart Language Quick-Reference (Dart 3.x)

```dart
// --- Null Safety ---
String? nullable;
String nonNull = nullable ?? 'default';
int length = nullable?.length ?? 0;
// nullable!.length;  // AVOID: force unwrap throws if null

// --- Records (Dart 3.0+) ---
(String, int) pair = ('hello', 42);
print(pair.$1); // 'hello'
print(pair.$2); // 42

({String name, int age}) person = (name: 'the user', age: 30);
print(person.name); // 'the user'

// Records in functions (return multiple values)
(User, String) loginUser(String email) {
  final user = User(email: email);
  final token = generateToken(user);
  return (user, token);
}
final (user, token) = loginUser('user@example.com');

// --- Patterns (Dart 3.0+) ---
// Destructuring
final (x, y) = (10, 20);
final {'name': name, 'age': age} = json;

// Switch expressions
String describe(Shape shape) => switch (shape) {
  Circle(radius: var r) when r > 10 => 'Large circle',
  Circle(radius: var r)             => 'Small circle (r=$r)',
  Square(side: var s)               => 'Square (s=$s)',
  Rectangle(width: var w, height: var h) => 'Rectangle ${w}x$h',
};

// If-case
if (json case {'user': {'name': String name}}) {
  print('Hello $name');
}

// --- Sealed Classes (Exhaustive Matching) ---
sealed class Result<T> {
  const Result();
}
class Success<T> extends Result<T> {
  final T data;
  const Success(this.data);
}
class Failure<T> extends Result<T> {
  final String message;
  final Exception? exception;
  const Failure(this.message, [this.exception]);
}

// Compiler enforces exhaustive matching:
String handle(Result<User> result) => switch (result) {
  Success(data: var user) => 'Got user: ${user.name}',
  Failure(message: var msg) => 'Error: $msg',
};

// --- Extension Methods ---
extension StringX on String {
  String get capitalize => isEmpty ? '' : '${this[0].toUpperCase()}${substring(1)}';
  bool get isEmail => RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(this);
}

extension ListX<T> on List<T> {
  List<T> sortedBy<K extends Comparable>(K Function(T) keyOf) {
    return [...this]..sort((a, b) => keyOf(a).compareTo(keyOf(b)));
  }
}

// --- Extension Types (Dart 3.3+) ---
extension type UserId(String value) {
  // Zero-cost wrapper: compiled away, no runtime overhead
  // But provides type safety at compile time
}
void deleteUser(UserId id) { /* ... */ }
// deleteUser('raw-string'); // COMPILE ERROR
deleteUser(UserId('user-123')); // OK

// --- Class Modifiers (Dart 3.0+) ---
interface class Printable { void printSelf(); }     // Can only be implemented
base class Animal { void breathe() {} }              // Can only be extended
final class DatabaseConnection { /* ... */ }          // Cannot be extended or implemented
mixin class Validator { bool validate() => true; }   // Can be used as both mixin and class
```

---

## 14. Flutter Debugging Protocol

### 14.1 Scientific Debugging for Flutter

Every Flutter bug follows this pipeline:

1. **CAPTURE** -- Copy exact error, full stack trace, reproduction steps
2. **REPRODUCE** -- Confirm the bug on device/emulator/web before touching code
3. **HYPOTHESIZE** -- Form a theory about the root cause (NOT the symptom)
4. **TEST** -- Validate hypothesis with debug tools (not guessing)
5. **FIX** -- Minimal change at the root cause
6. **VERIFY** -- Reproduce original steps, confirm bug is gone
7. **REGRESS** -- Run `flutter test` to ensure nothing else broke

### 14.2 DevTools Profiling

```bash
# ALWAYS profile in profile mode (debug mode adds overhead)
flutter run --profile

# Open DevTools
flutter pub run devtools
# Or: open DevTools from IDE (VS Code: Ctrl+Shift+P > "Open DevTools")
```

| DevTools Tab | Use When |
|-------------|----------|
| **Flutter Inspector** | Widget tree issues, layout problems, property inspection |
| **Performance** | Janky frames, slow animations, unnecessary rebuilds |
| **CPU Profiler** | Identifying slow functions, hot code paths |
| **Memory** | Leaks, growing allocations, retained objects |
| **Network** | API call failures, slow responses, payload inspection |
| **Logging** | Structured log analysis with `developer.log()` |
| **App Size** | Bundle bloat, large assets, tree-shaking issues |

**Enable Widget Rebuild Tracking**: DevTools > Performance > Track Widget Rebuilds
- Widgets that flash = unnecessary rebuilds = optimization targets

### 14.3 Programmatic Debugging

```dart
import 'dart:developer';
import 'package:flutter/foundation.dart';

// --- Structured Logging (categorized, DevTools-aware) ---
developer.log(
  'User logged in',
  name: 'auth.login',
  error: jsonEncode({'userId': user.id, 'method': 'email'}),
);

// --- Programmatic Breakpoints ---
debugger();                          // Unconditional breakpoint
debugger(when: offset > 30);         // Conditional breakpoint

// --- Dump Trees for Inspection ---
debugDumpApp();                      // Widget tree (hierarchy)
debugDumpRenderTree();               // Render tree (layout: sizes + constraints)
debugDumpLayerTree();                // Layer tree (compositing)
debugDumpFocusTree();                // Focus tree (keyboard navigation)
debugDumpSemanticsTree(DebugSemanticsDumpOrder.inverseHitTest); // Accessibility tree

// --- Debug Print (throttled, safe for large output) ---
debugPrint('Safe for large outputs — throttled to avoid dropped lines');

// --- Frame Timing ---
debugPrintBeginFrameBanner = true;   // Print frame start markers
debugPrintEndFrameBanner = true;     // Print frame end markers
```

### 14.4 Visual Debugging Flags

```dart
import 'package:flutter/rendering.dart';

// Layout debugging
debugPaintSizeEnabled = true;         // Shows widget boundaries (blue boxes)
debugPaintBaselinesEnabled = true;    // Shows text baselines
debugPaintPointersEnabled = true;     // Shows touch points
debugPaintLayerBordersEnabled = true; // Shows compositing layer boundaries
debugRepaintRainbowEnabled = true;    // Rainbow borders on repainted areas

// Performance overlay
MaterialApp(
  showPerformanceOverlay: true,       // GPU/UI thread frame times
  checkerboardRasterCacheImages: true, // Highlight cached images
  checkerboardOffscreenLayers: true,   // Highlight offscreen compositing
)
```

### 14.5 Custom Widget Diagnostics

```dart
class ProductCard extends StatelessWidget {
  final Product product;
  const ProductCard({super.key, required this.product});

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(StringProperty('name', product.name));
    properties.add(DoubleProperty('price', product.price));
    properties.add(FlagProperty('inStock',
      value: product.inStock,
      ifTrue: 'available',
      ifFalse: 'OUT OF STOCK',
    ));
  }

  @override
  Widget build(BuildContext context) => Card(child: Text(product.name));
}
```

### 14.6 Common Flutter Errors & Solutions

| Error | Root Cause | Fix |
|-------|-----------|-----|
| `RenderFlex overflowed by X pixels` | Content exceeds container bounds | Wrap in `SingleChildScrollView`, `Expanded`, or `Flexible` |
| `setState() called after dispose()` | Async callback fires after widget unmount | Check `mounted` before `setState`, or cancel async in `dispose()` |
| `A RenderFlex overflowed` in Row/Column | Children too wide/tall | Use `Expanded`/`Flexible`, or set `mainAxisSize: MainAxisSize.min` |
| `Null check operator used on a null value` | Force-unwrapping null | Use null-aware operators (`?.`, `??`), check for null before access |
| `Navigator operation requested with a context that does not include a Navigator` | Wrong BuildContext | Use `Builder` widget or pass correct context |
| `setState() or markNeedsBuild() called during build` | State change during build phase | Use `WidgetsBinding.instance.addPostFrameCallback` |
| `Failed assertion: !_debugLocked` | Modifying list during iteration | Copy list before modifying: `[...list]..remove(item)` |
| `HTTP connection failed` | Missing internet permission (Android) | Add `<uses-permission android:name="android.permission.INTERNET"/>` to AndroidManifest.xml |
| `MissingPluginException` | Plugin not registered for platform | Run `flutter clean && flutter pub get`, rebuild |
| `Vertical viewport was given unbounded height` | ListView inside Column without constraints | Wrap ListView in `Expanded` or set `shrinkWrap: true` |
| `Bad state: No element` | `.first`/`.single` on empty collection | Use `.firstOrNull` (Dart 3.0+) or check `.isNotEmpty` first |
| Hot reload not working | State stored in static/global vars | Move state into widget state or provider; restart instead of reload |

### 14.7 Memory Leak Detection

```dart
// Symptoms of memory leaks:
// - Memory tab in DevTools shows monotonically increasing memory
// - App gets slower over time
// - OOM crashes on lower-end devices

// Common causes & fixes:
// 1. StreamSubscription not cancelled
class _MyState extends State<MyWidget> {
  late final StreamSubscription _sub;

  @override
  void initState() {
    super.initState();
    _sub = myStream.listen((data) => setState(() => _data = data));
  }

  @override
  void dispose() {
    _sub.cancel();  // ALWAYS cancel subscriptions
    super.dispose();
  }
}

// 2. AnimationController not disposed
@override
void dispose() {
  _controller.dispose();  // ALWAYS dispose controllers
  super.dispose();
}

// 3. Timer not cancelled
late final Timer _timer;
@override
void dispose() {
  _timer.cancel();
  super.dispose();
}

// 4. Riverpod: Use autoDispose to avoid retention
final dataProvider = FutureProvider.autoDispose<Data>((ref) async {
  // Auto-disposed when no longer watched
  return fetchData();
});
```

### 14.8 Platform-Specific Debugging

```bash
# Android — Logcat
adb logcat -s flutter

# iOS — Console
open /Applications/Utilities/Console.app
# Filter by process name

# Web — Browser DevTools
# Chrome: F12 > Console tab
# Check `dart:html` and `dart:js_util` errors

# Native crash debugging
flutter run --verbose            # Verbose logging
flutter run --enable-software-rendering  # Bypass GPU issues
flutter run --trace-systrace     # System trace for native perf
```

### 14.9 Debugging Checklist

Before escalating any Flutter bug:

- [ ] Reproduced in debug mode with exact steps documented
- [ ] Checked `flutter doctor -v` for environment issues
- [ ] Ran `flutter clean && flutter pub get` to rule out cache issues
- [ ] Checked DevTools for the specific symptom (layout/perf/memory/network)
- [ ] Used `debugDumpApp()` or `debugDumpRenderTree()` if layout-related
- [ ] Checked `mounted` guards for async-after-dispose errors
- [ ] Verified all `dispose()` methods clean up subscriptions/controllers/timers
- [ ] Tested on BOTH Android and iOS (or web) if cross-platform

---

## 15. Verification Gates

Before declaring any Flutter task complete, ALL must pass:

- [ ] `flutter analyze` shows zero issues
- [ ] `flutter test` shows all tests passing (with specific test output shown)
- [ ] Widget tests exist for every new/modified screen
- [ ] Unit tests exist for every new/modified use case, repository, or notifier/bloc
- [ ] Both light AND dark themes visually verified
- [ ] No hardcoded strings (use `l10n` or constants file)
- [ ] No hardcoded colors (use `Theme.of(context).colorScheme`)
- [ ] No hardcoded sizes for text (use `Theme.of(context).textTheme`)
- [ ] Responsive layout tested at mobile, tablet, and desktop widths
- [ ] Performance profiled with DevTools (no janky frames)
- [ ] All `dispose()` methods properly clean up controllers and subscriptions

---

## 16. Anti-Premature-Completion

These do NOT count as evidence of completion:

| Claim | Why It Fails |
|-------|-------------|
| "It compiles" | Compilation does not verify logic or UI |
| "Tests pass" | Must show WHICH tests and their output |
| "No errors in analyze" | Does not check runtime behavior |
| "I added the widget" | Must show it rendering correctly |
| "Fixed the bug" | Must reproduce the original issue and show it no longer occurs |

What DOES count:

- Screenshot or emulator output showing the UI
- Test output with test names and pass/fail status
- `flutter analyze` output showing "No issues found!"
- Before/after comparison for bug fixes

---

## 17. 3-Strike Rule

After **3 consecutive failed attempts** at:
- Building the project (`flutter build`)
- Running tests (`flutter test`)
- Fixing the same error

**STOP.** Ask the user for clarification, device info, or environment details. Do not brute-force.

---

## 18. Common Packages

| Category | Package | Purpose |
|----------|---------|---------|
| State | `flutter_riverpod` | Riverpod state management |
| State | `flutter_bloc` | BLoC state management |
| Navigation | `go_router` | Declarative routing |
| Network | `dio` | HTTP client with interceptors |
| Network | `retrofit` | Type-safe REST client (code-gen) |
| JSON | `json_serializable` + `json_annotation` | JSON serialization |
| JSON | `freezed` | Immutable models + unions |
| DI | `get_it` + `injectable` | Service locator DI |
| Storage | `hive` / `isar` | Fast local NoSQL |
| Storage | `shared_preferences` | Simple key-value |
| Storage | `drift` | Reactive SQLite |
| Firebase | `firebase_core` + `cloud_firestore` | Backend-as-a-service |
| Auth | `firebase_auth` | Authentication |
| Images | `cached_network_image` | Image caching |
| UI | `google_fonts` | Custom fonts |
| UI | `flutter_animate` | Declarative animations |
| Testing | `mocktail` | Mocking (no codegen) |
| Testing | `bloc_test` | BLoC-specific testing |
| Build | `build_runner` | Code generation runner |
| Lint | `very_good_analysis` | Strict lint rules |

---

## 19. MCP Tool Integration (dart-mcp)

When MCP Dart tools are available, use them for faster feedback loops:

### Static Analysis & Code Quality

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `dart_analyzer` | Static analysis for errors, warnings, hints | After writing code, before running tests |
| `dart_format` | Apply consistent code formatting | After all code changes |
| `dart_fix` | Apply automated fixes for analyzer issues | When analyzer reports fixable issues |
| `dart_run_tests` | Execute Dart/Flutter tests | After writing tests, in TDD cycles |
| `dart_resolve_symbol` | Get symbol documentation and signatures | Understanding APIs, checking method signatures |
| `pub_dev_search` | Search pub.dev for packages | Finding packages for specific functionality |

### Runtime Debugging (requires running app)

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `get_runtime_errors` | Fetch current runtime errors and stack traces | First step in debugging, verify fixes |
| `get_widget_tree` | Inspect widget hierarchy at runtime | Layout debugging, finding widget issues |
| `hot_reload` | Apply code changes without losing state | After making fixes, during development |
| `hot_restart` | Full restart preserving debug session | When hot reload fails or state is corrupted |

### MCP Debug Workflow

```
1. get_runtime_errors → Capture current errors
2. get_widget_tree → Inspect UI structure
3. [make targeted fix]
4. hot_reload → Apply changes
5. get_runtime_errors → Verify fix (should be empty)
6. If hot_reload fails: dart_analyzer → fix errors → hot_restart
```

### MCP Pre-Commit Quality Check

```
1. dart_analyzer → Must show 0 errors, 0 warnings
2. dart_run_tests → All tests must pass
3. dart_format → Apply consistent formatting
4. dart_fix → Apply any remaining automated fixes
```

---

## 20. Flutter Code Review Protocol

When reviewing Flutter PRs, follow this priority order:

1. **Bugs/regressions**: state management, async flows, widget lifecycle, null safety, error handling, navigation, permissions, data persistence
2. **UX issues**: loading/empty/error states, accessibility, layout on small screens, gesture conflicts
3. **Performance**: rebuild hotspots, unnecessary `setState`, large lists without virtualization, expensive work on UI thread, image caching
4. **Tests**: ensure new logic is covered; run `flutter test` and `flutter analyze`
5. **Readability**: naming, file organization, dead code

**Output format**: Bullets ordered by severity with `path:line` references.

Example:
- `lib/screens/home_screen.dart:120` Potential NPE when `currentSong` is null; guard before accessing fields.
- `lib/widgets/player.dart:45` Missing `dispose()` for AnimationController — memory leak.

---

## 21. Advanced Testing Patterns

### Layer Isolation

| Layer | What to Test | What to Mock |
|-------|-------------|-------------|
| **Repository** | Data coordination between sources | DAOs, APIs, Logger |
| **DAO** | Database CRUD operations | Use real in-memory DB, mock Logger |
| **Provider** | State management and transitions | Services, Repositories |
| **Service** | Business logic and workflows | Repositories, Network clients |
| **Widget** | UI behaviour and interactions | Provider dependencies (via overrides) |

### Given-When-Then Structure

```dart
test('Given valid data, When fetchUsers called, Then returns user list', () async {
  // Arrange (Given)
  when(mockDAO.fetchAll()).thenAnswer((_) async => expectedUsers);

  // Act (When)
  final result = await repository.fetchUsers();

  // Assert (Then)
  expect(result, equals(expectedUsers));
  verify(mockDAO.fetchAll()).called(1);
});
```

### GetIt + Mockito Setup

```dart
@GenerateMocks([IUserDAO, IUserAPI, ILogger])
void main() {
  late MockIUserDAO mockDAO;

  setUp(() {
    mockDAO = MockIUserDAO();
    GetIt.I.registerSingleton<IUserDAO>(mockDAO);
  });

  tearDown(() => GetIt.I.reset()); // CRITICAL — always reset
}
// Run: dart run build_runner build (after modifying @GenerateMocks)
```

### Riverpod Provider Testing

```dart
ProviderContainer createContainer({List<Override> overrides = const []}) {
  final container = ProviderContainer(overrides: overrides);
  addTearDown(container.dispose);
  return container;
}

test('provider returns data', () async {
  final container = createContainer(overrides: [
    userServiceProvider.overrideWith((_) => MockUserService()),
  ]);
  // Never mock providers directly — override their dependencies
});
```

### Testing Common Mistakes

| Mistake | Fix |
|---------|-----|
| Mocking a provider directly | Override its dependencies: `provider.overrideWith(...)` |
| Missing `GetIt.I.reset()` in tearDown | Tests pollute each other — always reset |
| `await Future.delayed()` in tests | Use `tester.pumpAndSettle()` or `Completer` instead |
| Finding widgets by text | Use `find.byKey(const Key('name'))` — stable across text changes |
| No screen size in widget tests | Add `tester.view.physicalSize = const Size(1000, 1000)` |
| Not resetting platform override | Set `debugDefaultTargetPlatformOverride = null` at end |

### Test Checklist

- [ ] Dependencies mocked (not providers)
- [ ] SharedPreferences mocked if used
- [ ] `GetIt.I.reset()` in tearDown
- [ ] Streams closed in tearDown, controllers disposed
- [ ] Keys on source widgets, `find.byKey()` in tests
- [ ] Screen size set (`physicalSize` + `devicePixelRatio`)
- [ ] Success AND failure paths covered
- [ ] Edge cases tested (null, empty, max values)
- [ ] Loading and error states tested
- [ ] Given-When-Then naming used

---

## 22. Additional Error Patterns (Extended)

### BuildContext Errors

```dart
// Bad: Using context in initState
@override
void initState() {
  super.initState();
  // Theme.of(context); // ERROR!
}

// Good: Use didChangeDependencies
@override
void didChangeDependencies() {
  super.didChangeDependencies();
  final theme = Theme.of(context);
}

// Good: Use Builder for nested contexts
Scaffold(
  body: Builder(
    builder: (context) => ElevatedButton(
      onPressed: () => Scaffold.of(context).showSnackBar(...),
      child: Text('Show Snackbar'),
    ),
  ),
)
```

### CancelableOperation Pattern

```dart
CancelableOperation<Data>? _operation;

Future<void> fetchData() async {
  _operation = CancelableOperation.fromFuture(api.getData());
  final data = await _operation!.value;
  if (mounted) setState(() => _data = data);
}

@override
void dispose() {
  _operation?.cancel();
  super.dispose();
}
```

### RenderBox Was Not Laid Out

```dart
// Ensure parent provides constraints
SizedBox(width: 200, height: 200, child: CustomPaint(...))

// For intrinsic sizing
IntrinsicHeight(child: Row(children: [Container(), Container()]))
```

### Incorrect Use of ParentDataWidget

```dart
// Bad: Positioned outside Stack → ERROR
Column(children: [Positioned(...)])

// Good: Positioned inside Stack
Stack(children: [Positioned(top: 10, left: 10, child: Text('Hello'))])

// Bad: Expanded outside Flex → ERROR
Container(child: Expanded(...))

// Good: Expanded inside Row/Column
Row(children: [Expanded(child: Text('Hello'))])
```

### Global Error Handler

```dart
void main() {
  FlutterError.onError = (details) {
    FlutterError.presentError(details);
    crashReporter.recordFlutterError(details);
  };

  PlatformDispatcher.instance.onError = (error, stack) {
    crashReporter.recordError(error, stack);
    return true;
  };

  runApp(const MyApp());
}
```

---

## 23. Compound Skill Chaining

This skill auto-chains to other fireworks skills based on context:

| When You're Doing | Chain To | Why |
|---|---|---|
| Writing tests | `fireworks-test` | TDD methodology, edge case matrices |
| Debugging Flutter | `fireworks-debug` | Scientific 10-step protocol, root cause analysis |
| Security audit | `fireworks-security` | STRIDE threat model, CWE Top 25 + OWASP Mobile |
| Architecture decisions | `fireworks-architect` | RPI protocol, INVARIANTS, trade-off analysis |
| Performance issues | `fireworks-performance` | Profiling methodology, golden loop |
| Code review | `fireworks-review` | 6-lens analysis, severity scoring |
| Refactoring | `fireworks-refactor` | Safe refactoring, code smell catalog |

**Chaining is NOT optional.** If a Flutter task involves testing, you MUST also load `fireworks-test`. If it involves security, you MUST also load `fireworks-security`.

See `references/flutter-security-owasp.md` for OWASP Mobile Top 10 2024 with Flutter-specific mitigations.

---

## 24. Reference Files

| Reference | Contents |
|---|---|
| `references/riverpod-patterns.md` | Riverpod 3.0 -- codegen, families, lifecycle, pagination, auth guard |
| `references/riverpod-testing-guide.md` | Riverpod testing -- ProviderContainer overrides (NEVER mock providers), state mutations, lifecycle |
| `references/bloc-patterns.md` | BLoC 9.x -- Cubit, event transformers, Freezed, multi-BLoC, testing |
| `references/dart-modern-features.md` | Dart 3.7-3.10 -- wildcards, null-aware, dot shorthands, augmentations |
| `references/navigation-patterns.md` | GoRouter -- StatefulShellRoute, type-safe routes, deep linking |
| `references/animation-advanced.md` | Physics, CustomPainter, flutter_animate, Rive, TweenAnimationBuilder |
| `references/app-lifecycle.md` | AppLifecycleListener, RestorationMixin, restorationScopeId |
| `references/golden-testing.md` | Alchemist framework, CI mode, dark theme variants |
| `references/slivers-performance.md` | CustomScrollView, SliverFixedExtentList, Isolate.run, Impeller |
| `references/clean-architecture.md` | Feature-first clean architecture, repository pattern, DI, error handling |
| `references/widget-catalog.md` | Material 3 theming, responsive layouts, premium widget patterns |
| `references/testing-patterns.md` | Widget, unit, integration, golden, HTTP mocking, CI/CD |
| `references/debugging-patterns.md` | DevTools profiling, programmatic debugging, memory leak detection |
| `references/layer-testing-patterns.md` | Repository, DAO, Service layer isolation testing |
| `references/widget-testing-guide.md` | Widget tests -- interactions, dialogs, navigation, screen sizes |
| `references/flutter-security-owasp.md` | OWASP Mobile Top 10 2024 -- Flutter-specific mitigations, secure/insecure code |
