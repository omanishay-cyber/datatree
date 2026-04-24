# BLoC Pattern Reference for Flutter (flutter_bloc 9.x)

> Complete BLoC and Cubit patterns including event/state architecture,
> event transformers, provider patterns, observer, testing, Freezed integration,
> repository pattern, error handling, nested BLoCs, and migration from Provider.

---

## 1. BLoC vs Cubit Decision Tree

```
Do you need event-driven architecture?
├── YES → Use BLoC
│   ├── Complex event transformers (debounce, throttle)?
│   ├── Event replay / undo-redo needed?
│   ├── Multiple events producing same state change?
│   └── Need to trace exactly which event caused a state change?
│
└── NO → Use Cubit
    ├── Simple state changes (toggle, increment, set value)?
    ├── Direct method calls are sufficient?
    ├── No need for event history or replay?
    └── Fewer boilerplate is preferred?

Rule of thumb:
- Start with Cubit. Upgrade to BLoC when complexity demands it.
- Cubit = methods that emit states directly.
- BLoC = events in, states out, with transformers in between.
```

---

## 2. Cubit — Simple State Management

### Basic Cubit with Primitive State

```dart
class CounterCubit extends Cubit<int> {
  CounterCubit() : super(0);

  void increment() => emit(state + 1);
  void decrement() => emit(state - 1);
  void reset() => emit(0);
}
```

### Cubit with Complex State and copyWith

```dart
class ProfileState {
  final String name;
  final String email;
  final bool isLoading;
  final String? error;

  const ProfileState({
    this.name = '',
    this.email = '',
    this.isLoading = false,
    this.error,
  });

  ProfileState copyWith({
    String? name,
    String? email,
    bool? isLoading,
    String? error,
  }) {
    return ProfileState(
      name: name ?? this.name,
      email: email ?? this.email,
      isLoading: isLoading ?? this.isLoading,
      error: error,
    );
  }
}

class ProfileCubit extends Cubit<ProfileState> {
  final ProfileRepository _repository;

  ProfileCubit(this._repository) : super(const ProfileState());

  Future<void> loadProfile(String userId) async {
    emit(state.copyWith(isLoading: true, error: null));
    try {
      final profile = await _repository.getProfile(userId);
      emit(state.copyWith(
        name: profile.name,
        email: profile.email,
        isLoading: false,
      ));
    } catch (e) {
      emit(state.copyWith(isLoading: false, error: e.toString()));
    }
  }

  void updateName(String name) => emit(state.copyWith(name: name));
  void updateEmail(String email) => emit(state.copyWith(email: email));
}
```

---

## 3. BLoC — Event-Driven State Management

### Events with Sealed Classes

```dart
sealed class AuthEvent {}

class LoginRequested extends AuthEvent {
  final String email;
  final String password;
  LoginRequested({required this.email, required this.password});
}

class LogoutRequested extends AuthEvent {}

class AuthCheckRequested extends AuthEvent {}

class TokenRefreshRequested extends AuthEvent {}
```

### States with Sealed Classes

```dart
sealed class AuthState {}

class AuthInitial extends AuthState {}

class AuthLoading extends AuthState {}

class AuthSuccess extends AuthState {
  final User user;
  AuthSuccess(this.user);
}

class AuthFailure extends AuthState {
  final String message;
  final bool canRetry;
  AuthFailure(this.message, {this.canRetry = true});
}
```

### BLoC Implementation

```dart
class AuthBloc extends Bloc<AuthEvent, AuthState> {
  final AuthRepository _authRepository;

  AuthBloc(this._authRepository) : super(AuthInitial()) {
    on<LoginRequested>(_onLoginRequested);
    on<LogoutRequested>(_onLogoutRequested);
    on<AuthCheckRequested>(_onAuthCheck);
    on<TokenRefreshRequested>(_onTokenRefresh);
  }

  Future<void> _onLoginRequested(
    LoginRequested event,
    Emitter<AuthState> emit,
  ) async {
    emit(AuthLoading());
    try {
      final user = await _authRepository.login(
        email: event.email,
        password: event.password,
      );
      emit(AuthSuccess(user));
    } on InvalidCredentialsException {
      emit(AuthFailure('Invalid email or password'));
    } on NetworkException {
      emit(AuthFailure('No internet connection', canRetry: true));
    } catch (e) {
      emit(AuthFailure('Unexpected error: ${e.toString()}'));
    }
  }

  Future<void> _onLogoutRequested(
    LogoutRequested event,
    Emitter<AuthState> emit,
  ) async {
    await _authRepository.logout();
    emit(AuthInitial());
  }

  Future<void> _onAuthCheck(
    AuthCheckRequested event,
    Emitter<AuthState> emit,
  ) async {
    final user = await _authRepository.getCurrentUser();
    if (user != null) {
      emit(AuthSuccess(user));
    } else {
      emit(AuthInitial());
    }
  }

  Future<void> _onTokenRefresh(
    TokenRefreshRequested event,
    Emitter<AuthState> emit,
  ) async {
    try {
      await _authRepository.refreshToken();
    } catch (e) {
      emit(AuthInitial()); // Force re-login
    }
  }
}
```

---

## 4. Event Transformers (bloc_concurrency)

```yaml
# pubspec.yaml
dependencies:
  bloc_concurrency: ^0.2.0
```

```dart
import 'package:bloc_concurrency/bloc_concurrency.dart';
import 'package:stream_transform/stream_transform.dart';

class SearchBloc extends Bloc<SearchEvent, SearchState> {
  SearchBloc(this._repository) : super(SearchInitial()) {
    // Sequential: process events one at a time, queue the rest
    on<SearchSubmitted>(_onSubmitted, transformer: sequential());

    // Concurrent: process all events simultaneously
    on<SearchResultTapped>(_onResultTapped, transformer: concurrent());

    // Droppable: ignore new events while processing current one
    on<SearchAutoComplete>(_onAutoComplete, transformer: droppable());

    // Restartable: cancel current processing when new event arrives
    on<SearchQueryChanged>(_onQueryChanged, transformer: restartable());
  }

  // Custom debounce transformer
  on<SearchQueryChanged>(
    _onQueryChanged,
    transformer: (events, mapper) => events
        .debounce(const Duration(milliseconds: 300))
        .switchMap(mapper),
  );

  // Custom throttle transformer
  on<ScrollPositionChanged>(
    _onScrollChanged,
    transformer: (events, mapper) => events
        .throttle(const Duration(milliseconds: 100))
        .switchMap(mapper),
  );
}
```

### Transformer Decision Guide

```
sequential()   — Form submissions, order placement (must process in order)
concurrent()   — Independent actions (tapping items, toggling favorites)
droppable()    — Pull-to-refresh (ignore rapid repeated triggers)
restartable()  — Search-as-you-type (cancel previous, start fresh)
debounce       — Text input validation (wait for user to stop typing)
throttle       — Scroll events, resize events (limit rate)
```

---

## 5. BlocProvider, MultiBlocProvider, BlocBuilder, BlocListener, BlocConsumer

### BlocProvider

```dart
// Create and provide a new BLoC
BlocProvider(
  create: (context) => AuthBloc(context.read<AuthRepository>())
    ..add(AuthCheckRequested()),
  child: const LoginPage(),
)

// Provide existing BLoC instance (e.g., sharing across routes)
BlocProvider.value(
  value: existingBloc,
  child: const DetailPage(),
)

// Multiple providers
MultiBlocProvider(
  providers: [
    BlocProvider(create: (_) => AuthBloc(getIt<AuthRepository>())),
    BlocProvider(create: (_) => ThemeCubit()),
    BlocProvider(create: (ctx) => CartBloc(getIt<CartRepository>())),
  ],
  child: const MyApp(),
)

// Accessing BLoCs
context.read<AuthBloc>();              // One-time read (use in callbacks)
context.watch<AuthBloc>().state;       // Rebuilds on every state change
context.select<AuthBloc, bool>(        // Selective rebuild
  (bloc) => bloc.state is AuthLoading,
);
```

### BlocBuilder

```dart
BlocBuilder<AuthBloc, AuthState>(
  builder: (context, state) {
    return switch (state) {
      AuthInitial()   => const LoginForm(),
      AuthLoading()   => const CircularProgressIndicator(),
      AuthSuccess(:final user) => HomeScreen(user: user),
      AuthFailure(:final message) => ErrorWidget(message: message),
    };
  },
)

// Conditional rebuilds with buildWhen
BlocBuilder<AuthBloc, AuthState>(
  buildWhen: (previous, current) =>
      previous.runtimeType != current.runtimeType,
  builder: (context, state) { /* ... */ },
)
```

### BlocListener

```dart
BlocListener<AuthBloc, AuthState>(
  listener: (context, state) {
    if (state is AuthFailure) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(state.message)),
      );
    }
    if (state is AuthSuccess) {
      context.go('/home');
    }
  },
  child: const LoginPage(),
)

// Multiple listeners
MultiBlocListener(
  listeners: [
    BlocListener<AuthBloc, AuthState>(
      listener: (context, state) { /* handle auth state */ },
    ),
    BlocListener<ConnectivityCubit, ConnectivityState>(
      listener: (context, state) { /* handle connectivity */ },
    ),
  ],
  child: const AppShell(),
)

// Conditional listening with listenWhen
BlocListener<AuthBloc, AuthState>(
  listenWhen: (previous, current) => current is AuthFailure,
  listener: (context, state) { /* only fires on AuthFailure */ },
  child: const LoginPage(),
)
```

### BlocConsumer (Builder + Listener Combined)

```dart
BlocConsumer<AuthBloc, AuthState>(
  listenWhen: (previous, current) => current is AuthFailure,
  listener: (context, state) {
    if (state is AuthFailure) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(state.message)),
      );
    }
  },
  buildWhen: (previous, current) => current is! AuthFailure,
  builder: (context, state) {
    return switch (state) {
      AuthLoading() => const CircularProgressIndicator(),
      AuthSuccess(:final user) => WelcomeText(user: user),
      _ => const LoginForm(),
    };
  },
)
```

---

## 6. BlocObserver for Logging and Analytics

```dart
class AppBlocObserver extends BlocObserver {
  @override
  void onCreate(BlocBase bloc) {
    super.onCreate(bloc);
    debugPrint('[BLoC Created] ${bloc.runtimeType}');
  }

  @override
  void onEvent(Bloc bloc, Object? event) {
    super.onEvent(bloc, event);
    debugPrint('[Event] ${bloc.runtimeType} -> $event');
  }

  @override
  void onTransition(Bloc bloc, Transition transition) {
    super.onTransition(bloc, transition);
    debugPrint(
      '[Transition] ${bloc.runtimeType}: '
      '${transition.currentState.runtimeType} -> '
      '${transition.nextState.runtimeType}',
    );
  }

  @override
  void onChange(BlocBase bloc, Change change) {
    super.onChange(bloc, change);
    debugPrint(
      '[Change] ${bloc.runtimeType}: '
      '${change.currentState.runtimeType} -> '
      '${change.nextState.runtimeType}',
    );
  }

  @override
  void onError(BlocBase bloc, Object error, StackTrace stackTrace) {
    super.onError(bloc, error, stackTrace);
    debugPrint('[Error] ${bloc.runtimeType}: $error');
    // Send to crash reporting: Sentry, Crashlytics, etc.
  }

  @override
  void onClose(BlocBase bloc) {
    super.onClose(bloc);
    debugPrint('[BLoC Closed] ${bloc.runtimeType}');
  }
}

// Register in main.dart
void main() {
  Bloc.observer = AppBlocObserver();
  runApp(const MyApp());
}
```

---

## 7. Testing BLoCs

```dart
import 'package:bloc_test/bloc_test.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';

class MockAuthRepository extends Mock implements AuthRepository {}

void main() {
  late MockAuthRepository mockRepo;

  setUp(() {
    mockRepo = MockAuthRepository();
  });

  group('AuthBloc', () {
    test('initial state is AuthInitial', () {
      final bloc = AuthBloc(mockRepo);
      expect(bloc.state, isA<AuthInitial>());
      bloc.close();
    });

    blocTest<AuthBloc, AuthState>(
      'emits [AuthLoading, AuthSuccess] on successful login',
      build: () {
        when(() => mockRepo.login(
          email: any(named: 'email'),
          password: any(named: 'password'),
        )).thenAnswer((_) async => User(id: '1', name: 'Test'));
        return AuthBloc(mockRepo);
      },
      act: (bloc) => bloc.add(
        LoginRequested(email: 'test@test.com', password: 'pass123'),
      ),
      expect: () => [
        isA<AuthLoading>(),
        isA<AuthSuccess>().having((s) => s.user.name, 'name', 'Test'),
      ],
      verify: (_) {
        verify(() => mockRepo.login(
          email: 'test@test.com',
          password: 'pass123',
        )).called(1);
      },
    );

    blocTest<AuthBloc, AuthState>(
      'emits [AuthLoading, AuthFailure] on invalid credentials',
      build: () {
        when(() => mockRepo.login(
          email: any(named: 'email'),
          password: any(named: 'password'),
        )).thenThrow(InvalidCredentialsException());
        return AuthBloc(mockRepo);
      },
      act: (bloc) => bloc.add(
        LoginRequested(email: 'bad@test.com', password: 'wrong'),
      ),
      expect: () => [
        isA<AuthLoading>(),
        isA<AuthFailure>().having(
          (s) => s.message,
          'message',
          'Invalid email or password',
        ),
      ],
    );

    blocTest<AuthBloc, AuthState>(
      'emits [AuthInitial] on logout',
      build: () {
        when(() => mockRepo.logout()).thenAnswer((_) async {});
        return AuthBloc(mockRepo);
      },
      seed: () => AuthSuccess(User(id: '1', name: 'Test')),
      act: (bloc) => bloc.add(LogoutRequested()),
      expect: () => [isA<AuthInitial>()],
    );
  });
}
```

---

## 8. Freezed Integration for Immutable Events and States

```yaml
# pubspec.yaml
dependencies:
  freezed_annotation: ^2.4.0
dev_dependencies:
  freezed: ^2.5.0
  build_runner: ^2.4.0
```

```dart
import 'package:freezed_annotation/freezed_annotation.dart';

part 'auth_event.freezed.dart';
part 'auth_state.freezed.dart';

// Events with Freezed
@freezed
sealed class AuthEvent with _$AuthEvent {
  const factory AuthEvent.loginRequested({
    required String email,
    required String password,
  }) = LoginRequested;

  const factory AuthEvent.logoutRequested() = LogoutRequested;
  const factory AuthEvent.authCheckRequested() = AuthCheckRequested;
}

// States with Freezed
@freezed
sealed class AuthState with _$AuthState {
  const factory AuthState.initial() = AuthInitial;
  const factory AuthState.loading() = AuthLoading;
  const factory AuthState.success({required User user}) = AuthSuccess;
  const factory AuthState.failure({
    required String message,
    @Default(true) bool canRetry,
  }) = AuthFailure;
}

// Usage in BLoC handler with pattern matching
Future<void> _onLoginRequested(
  LoginRequested event,
  Emitter<AuthState> emit,
) async {
  emit(const AuthState.loading());
  try {
    final user = await _repo.login(email: event.email, password: event.password);
    emit(AuthState.success(user: user));
  } catch (e) {
    emit(AuthState.failure(message: e.toString()));
  }
}

// Pattern matching on state in UI
builder: (context, state) {
  return state.when(
    initial: () => const LoginForm(),
    loading: () => const CircularProgressIndicator(),
    success: (user) => HomeScreen(user: user),
    failure: (message, canRetry) => ErrorView(
      message: message,
      showRetry: canRetry,
    ),
  );
}
```

Run code generation:
```bash
dart run build_runner build --delete-conflicting-outputs
```

---

## 9. Repository Pattern with BLoC

```
BLoC → Repository Interface → Repository Implementation → DataSource
                                                          ├── RemoteDataSource (API)
                                                          └── LocalDataSource (Cache/DB)
```

```dart
// Domain layer: repository interface
abstract class ProductRepository {
  Future<List<Product>> getAll();
  Future<Product> getById(String id);
  Future<Product> create(Product product);
  Future<void> delete(String id);
  Stream<List<Product>> watchAll();
}

// Data layer: repository implementation
class ProductRepositoryImpl implements ProductRepository {
  final ProductRemoteDataSource _remote;
  final ProductLocalDataSource _local;

  ProductRepositoryImpl(this._remote, this._local);

  @override
  Future<List<Product>> getAll() async {
    try {
      final products = await _remote.fetchAll();
      await _local.cacheProducts(products);
      return products;
    } on NetworkException {
      return _local.getCachedProducts();
    }
  }

  @override
  Stream<List<Product>> watchAll() {
    return _local.watchProducts();
  }
}

// BLoC uses repository interface (depends on abstraction)
class ProductBloc extends Bloc<ProductEvent, ProductState> {
  final ProductRepository _repository; // Interface, not implementation

  ProductBloc(this._repository) : super(ProductInitial()) {
    on<ProductsFetched>(_onFetched);
  }
}
```

---

## 10. Error Handling Patterns in BLoC

```dart
// Typed error state with retry capability
sealed class DataState {}
class DataInitial extends DataState {}
class DataLoading extends DataState {}
class DataLoaded extends DataState {
  final List<Item> items;
  DataLoaded(this.items);
}
class DataError extends DataState {
  final String message;
  final ErrorType type;
  final bool canRetry;
  DataError({required this.message, required this.type, this.canRetry = false});
}

enum ErrorType { network, server, auth, validation, unknown }

// Handler with granular error catching
Future<void> _onFetched(DataFetched event, Emitter<DataState> emit) async {
  emit(DataLoading());
  try {
    final items = await _repository.getAll();
    emit(DataLoaded(items));
  } on SocketException {
    emit(DataError(message: 'No internet', type: ErrorType.network, canRetry: true));
  } on HttpException catch (e) {
    emit(DataError(
      message: 'Server error: ${e.message}',
      type: ErrorType.server,
      canRetry: true,
    ));
  } on UnauthorizedException {
    emit(DataError(message: 'Session expired', type: ErrorType.auth));
  } catch (e, stack) {
    log('Unexpected error', error: e, stackTrace: stack);
    emit(DataError(message: 'Something went wrong', type: ErrorType.unknown, canRetry: true));
  }
}
```

---

## 11. Nested BLoCs and Communication Between BLoCs

### Pattern A: Parent BLoC Listens to Child BLoC Stream

```dart
class OrderBloc extends Bloc<OrderEvent, OrderState> {
  final CartBloc _cartBloc;
  late final StreamSubscription<CartState> _cartSub;

  OrderBloc({required CartBloc cartBloc})
      : _cartBloc = cartBloc,
        super(OrderInitial()) {
    on<OrderSubmitted>(_onSubmitted);
    on<CartStateChanged>(_onCartChanged);

    _cartSub = _cartBloc.stream.listen((cartState) {
      if (cartState is CartLoaded) {
        add(CartStateChanged(cartState.items));
      }
    });
  }

  @override
  Future<void> close() {
    _cartSub.cancel();
    return super.close();
  }
}
```

### Pattern B: Shared Repository Stream (Preferred)

```dart
// Both BLoCs subscribe to the same repository stream
// No direct BLoC-to-BLoC dependency
class CartRepository {
  final _controller = StreamController<List<CartItem>>.broadcast();
  Stream<List<CartItem>> get itemsStream => _controller.stream;

  void addItem(CartItem item) {
    _items.add(item);
    _controller.add(List.unmodifiable(_items));
  }
}

class CartBloc extends Bloc<CartEvent, CartState> {
  CartBloc(CartRepository repo) : super(CartInitial()) {
    // Listens to repo stream
  }
}

class CheckoutBloc extends Bloc<CheckoutEvent, CheckoutState> {
  CheckoutBloc(CartRepository repo) : super(CheckoutInitial()) {
    // Also listens to same repo stream — no BLoC coupling
  }
}
```

### Pattern C: UI Mediates Communication

```dart
// Parent widget coordinates two BLoCs via listeners
BlocListener<CartBloc, CartState>(
  listener: (context, cartState) {
    if (cartState is CartUpdated) {
      context.read<PricingBloc>().add(RecalculateTotal(cartState.items));
    }
  },
  child: const CheckoutPage(),
)
```

---

## 12. Migration from Provider to BLoC

### Before (Provider / ChangeNotifier)

```dart
class CounterProvider extends ChangeNotifier {
  int _count = 0;
  int get count => _count;

  void increment() {
    _count++;
    notifyListeners();
  }
}

// In widget tree
ChangeNotifierProvider(
  create: (_) => CounterProvider(),
  child: Consumer<CounterProvider>(
    builder: (context, counter, _) => Text('${counter.count}'),
  ),
)
```

### After (BLoC / Cubit)

```dart
class CounterCubit extends Cubit<int> {
  CounterCubit() : super(0);
  void increment() => emit(state + 1);
}

// In widget tree
BlocProvider(
  create: (_) => CounterCubit(),
  child: BlocBuilder<CounterCubit, int>(
    builder: (context, count) => Text('$count'),
  ),
)
```

### Migration Mapping

```
Provider concept          →  BLoC equivalent
──────────────────────────────────────────────
ChangeNotifierProvider    →  BlocProvider
MultiProvider             →  MultiBlocProvider
Consumer                  →  BlocBuilder
Selector                  →  BlocSelector / buildWhen
context.watch<T>()        →  context.watch<T>() (same API)
context.read<T>()         →  context.read<T>() (same API)
ChangeNotifier            →  Cubit (simple) or Bloc (complex)
notifyListeners()         →  emit(newState)
ProxyProvider             →  BlocListener + context.read
```

### Step-by-Step Migration

1. Add `flutter_bloc` to pubspec.yaml
2. Convert ChangeNotifiers to Cubits (1:1 mapping)
3. Replace `ChangeNotifierProvider` with `BlocProvider`
4. Replace `Consumer` with `BlocBuilder`
5. Replace `Selector` with `BlocBuilder` + `buildWhen` or `context.select`
6. Move side effects from `Consumer` to `BlocListener`
7. Remove `provider` package after full migration
8. Consider upgrading Cubits to BLoCs where event tracing adds value
