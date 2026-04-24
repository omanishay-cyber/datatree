# Clean Architecture for Flutter (Domain-Driven Design)

> Three-layer architecture separating concerns: Presentation, Domain, and Data.
> Covers entities, use cases, repositories, DTOs, dependency injection,
> functional error handling with Either, and pragmatic shortcuts.

---

## 1. The Three Layers

```
┌─────────────────────────────────────────────┐
│              Presentation Layer             │
│   Pages, Widgets, BLoC/Riverpod, ViewModels │
│   Depends on: Domain                        │
├─────────────────────────────────────────────┤
│               Domain Layer                  │
│   Entities, Use Cases, Repository Interfaces│
│   Depends on: Nothing (pure Dart)           │
├─────────────────────────────────────────────┤
│                Data Layer                   │
│   Repositories Impl, Data Sources, Models   │
│   Depends on: Domain                        │
└─────────────────────────────────────────────┘

Dependency Rule:
- Domain depends on NOTHING
- Presentation depends on Domain
- Data depends on Domain (implements interfaces)
- Data and Presentation NEVER depend on each other
```

---

## 2. Feature-First Folder Structure

```
lib/
├── core/
│   ├── error/
│   │   ├── exceptions.dart          # Raw exceptions (throw these)
│   │   └── failures.dart            # Failure classes (return these)
│   ├── network/
│   │   ├── api_client.dart          # Dio/http setup
│   │   └── network_info.dart        # Connectivity checker
│   ├── usecases/
│   │   └── usecase.dart             # Base UseCase class
│   ├── constants/
│   │   └── api_constants.dart
│   └── utils/
│       ├── input_converter.dart
│       └── date_formatter.dart
│
├── features/
│   ├── auth/
│   │   ├── domain/
│   │   │   ├── entities/
│   │   │   │   └── user.dart
│   │   │   ├── repositories/
│   │   │   │   └── auth_repository.dart       # Abstract class
│   │   │   └── usecases/
│   │   │       ├── login.dart
│   │   │       ├── logout.dart
│   │   │       └── get_current_user.dart
│   │   ├── data/
│   │   │   ├── models/
│   │   │   │   └── user_model.dart            # Extends/implements entity
│   │   │   ├── repositories/
│   │   │   │   └── auth_repository_impl.dart  # Implements interface
│   │   │   └── datasources/
│   │   │       ├── auth_remote_datasource.dart
│   │   │       └── auth_local_datasource.dart
│   │   └── presentation/
│   │       ├── pages/
│   │       │   ├── login_page.dart
│   │       │   └── register_page.dart
│   │       ├── widgets/
│   │       │   ├── login_form.dart
│   │       │   └── social_login_buttons.dart
│   │       └── bloc/
│   │           ├── auth_bloc.dart
│   │           ├── auth_event.dart
│   │           └── auth_state.dart
│   │
│   ├── products/
│   │   ├── domain/
│   │   ├── data/
│   │   └── presentation/
│   │
│   └── orders/
│       ├── domain/
│       ├── data/
│       └── presentation/
│
├── injection_container.dart         # get_it setup
└── main.dart
```

---

## 3. Domain Layer

### Entities -- Pure Dart Classes

```dart
// lib/features/auth/domain/entities/user.dart
class User {
  final String id;
  final String name;
  final String email;
  final UserRole role;

  const User({
    required this.id,
    required this.name,
    required this.email,
    required this.role,
  });
}

enum UserRole { admin, manager, staff }
```

Entities contain ONLY business logic and properties. No framework dependencies.
No `fromJson`, no `toJson`, no annotations.

### Repository Interfaces -- Contracts

```dart
// lib/features/auth/domain/repositories/auth_repository.dart
import 'package:fpdart/fpdart.dart';

abstract class AuthRepository {
  Future<Either<Failure, User>> login({
    required String email,
    required String password,
  });
  Future<Either<Failure, Unit>> logout();
  Future<Either<Failure, User>> getCurrentUser();
  Future<Either<Failure, User>> register({
    required String name,
    required String email,
    required String password,
  });
}
```

### Use Cases -- Single Responsibility

```dart
// lib/core/usecases/usecase.dart
import 'package:fpdart/fpdart.dart';

abstract class UseCase<Type, Params> {
  Future<Either<Failure, Type>> call(Params params);
}

class NoParams {}
```

```dart
// lib/features/auth/domain/usecases/login.dart
class Login implements UseCase<User, LoginParams> {
  final AuthRepository repository;

  Login(this.repository);

  @override
  Future<Either<Failure, User>> call(LoginParams params) {
    return repository.login(
      email: params.email,
      password: params.password,
    );
  }
}

class LoginParams {
  final String email;
  final String password;

  const LoginParams({required this.email, required this.password});
}
```

```dart
// lib/features/auth/domain/usecases/get_current_user.dart
class GetCurrentUser implements UseCase<User, NoParams> {
  final AuthRepository repository;

  GetCurrentUser(this.repository);

  @override
  Future<Either<Failure, User>> call(NoParams params) {
    return repository.getCurrentUser();
  }
}
```

---

## 4. Data Layer

### Models -- DTOs with Serialization

```dart
// lib/features/auth/data/models/user_model.dart
class UserModel extends User {
  const UserModel({
    required super.id,
    required super.name,
    required super.email,
    required super.role,
  });

  factory UserModel.fromJson(Map<String, dynamic> json) {
    return UserModel(
      id: json['id'] as String,
      name: json['name'] as String,
      email: json['email'] as String,
      role: UserRole.values.byName(json['role'] as String),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'name': name,
      'email': email,
      'role': role.name,
    };
  }

  factory UserModel.fromEntity(User user) {
    return UserModel(
      id: user.id,
      name: user.name,
      email: user.email,
      role: user.role,
    );
  }
}
```

### Data Sources

```dart
// lib/features/auth/data/datasources/auth_remote_datasource.dart
abstract class AuthRemoteDataSource {
  Future<UserModel> login({required String email, required String password});
  Future<void> logout();
  Future<UserModel> getCurrentUser();
}

class AuthRemoteDataSourceImpl implements AuthRemoteDataSource {
  final Dio dio;

  AuthRemoteDataSourceImpl(this.dio);

  @override
  Future<UserModel> login({
    required String email,
    required String password,
  }) async {
    final response = await dio.post(
      '/auth/login',
      data: {'email': email, 'password': password},
    );

    if (response.statusCode == 200) {
      return UserModel.fromJson(response.data['user']);
    } else {
      throw ServerException(
        message: response.data['message'] ?? 'Login failed',
        statusCode: response.statusCode ?? 500,
      );
    }
  }
}

// lib/features/auth/data/datasources/auth_local_datasource.dart
abstract class AuthLocalDataSource {
  Future<UserModel?> getCachedUser();
  Future<void> cacheUser(UserModel user);
  Future<void> clearCache();
}

class AuthLocalDataSourceImpl implements AuthLocalDataSource {
  final SharedPreferences prefs;
  static const _userKey = 'cached_user';

  AuthLocalDataSourceImpl(this.prefs);

  @override
  Future<UserModel?> getCachedUser() async {
    final jsonString = prefs.getString(_userKey);
    if (jsonString == null) return null;
    return UserModel.fromJson(json.decode(jsonString));
  }

  @override
  Future<void> cacheUser(UserModel user) async {
    await prefs.setString(_userKey, json.encode(user.toJson()));
  }

  @override
  Future<void> clearCache() async {
    await prefs.remove(_userKey);
  }
}
```

### Repository Implementation

```dart
// lib/features/auth/data/repositories/auth_repository_impl.dart
class AuthRepositoryImpl implements AuthRepository {
  final AuthRemoteDataSource remoteDataSource;
  final AuthLocalDataSource localDataSource;
  final NetworkInfo networkInfo;

  AuthRepositoryImpl({
    required this.remoteDataSource,
    required this.localDataSource,
    required this.networkInfo,
  });

  @override
  Future<Either<Failure, User>> login({
    required String email,
    required String password,
  }) async {
    if (await networkInfo.isConnected) {
      try {
        final userModel = await remoteDataSource.login(
          email: email,
          password: password,
        );
        await localDataSource.cacheUser(userModel);
        return Right(userModel);
      } on ServerException catch (e) {
        return Left(ServerFailure(e.message));
      } on DioException catch (e) {
        return Left(NetworkFailure(e.message ?? 'Network error'));
      }
    } else {
      return const Left(NetworkFailure('No internet connection'));
    }
  }

  @override
  Future<Either<Failure, User>> getCurrentUser() async {
    try {
      if (await networkInfo.isConnected) {
        final user = await remoteDataSource.getCurrentUser();
        await localDataSource.cacheUser(user);
        return Right(user);
      } else {
        final cachedUser = await localDataSource.getCachedUser();
        if (cachedUser != null) {
          return Right(cachedUser);
        }
        return const Left(CacheFailure('No cached user found'));
      }
    } on ServerException catch (e) {
      return Left(ServerFailure(e.message));
    }
  }
}
```

---

## 5. Error Handling with Either (fpdart)

```yaml
# pubspec.yaml
dependencies:
  fpdart: ^1.1.0
```

### Failure Classes Hierarchy

```dart
// lib/core/error/failures.dart
sealed class Failure {
  final String message;
  const Failure(this.message);
}

class ServerFailure extends Failure {
  final int? statusCode;
  const ServerFailure(super.message, {this.statusCode});
}

class CacheFailure extends Failure {
  const CacheFailure(super.message);
}

class NetworkFailure extends Failure {
  const NetworkFailure(super.message);
}

class ValidationFailure extends Failure {
  final Map<String, String> fieldErrors;
  const ValidationFailure(super.message, {this.fieldErrors = const {}});
}

class AuthFailure extends Failure {
  const AuthFailure(super.message);
}
```

### Exception Classes

```dart
// lib/core/error/exceptions.dart
class ServerException implements Exception {
  final String message;
  final int statusCode;
  const ServerException({required this.message, required this.statusCode});
}

class CacheException implements Exception {
  final String message;
  const CacheException(this.message);
}
```

### Using Either in BLoC

```dart
class AuthBloc extends Bloc<AuthEvent, AuthState> {
  final Login _login;

  AuthBloc({required Login login})
      : _login = login,
        super(AuthInitial()) {
    on<LoginRequested>(_onLoginRequested);
  }

  Future<void> _onLoginRequested(
    LoginRequested event,
    Emitter<AuthState> emit,
  ) async {
    emit(AuthLoading());

    final result = await _login(LoginParams(
      email: event.email,
      password: event.password,
    ));

    // fold: Left -> failure handler, Right -> success handler
    result.fold(
      (failure) => emit(AuthError(failure.message)),
      (user) => emit(AuthAuthenticated(user)),
    );

    // Alternative with pattern matching
    switch (result) {
      case Left(value: final failure):
        emit(AuthError(failure.message));
      case Right(value: final user):
        emit(AuthAuthenticated(user));
    }
  }
}
```

---

## 6. Dependency Injection with get_it and injectable

### Manual get_it Setup

```dart
// lib/injection_container.dart
import 'package:get_it/get_it.dart';

final getIt = GetIt.instance;

Future<void> initDependencies() async {
  // External
  final prefs = await SharedPreferences.getInstance();
  getIt.registerSingleton<SharedPreferences>(prefs);

  final dio = Dio(BaseOptions(baseUrl: ApiConstants.baseUrl));
  getIt.registerSingleton<Dio>(dio);

  getIt.registerSingleton<NetworkInfo>(NetworkInfoImpl());

  // Data sources
  getIt.registerLazySingleton<AuthRemoteDataSource>(
    () => AuthRemoteDataSourceImpl(getIt<Dio>()),
  );
  getIt.registerLazySingleton<AuthLocalDataSource>(
    () => AuthLocalDataSourceImpl(getIt<SharedPreferences>()),
  );

  // Repositories
  getIt.registerLazySingleton<AuthRepository>(
    () => AuthRepositoryImpl(
      remoteDataSource: getIt<AuthRemoteDataSource>(),
      localDataSource: getIt<AuthLocalDataSource>(),
      networkInfo: getIt<NetworkInfo>(),
    ),
  );

  // Use cases
  getIt.registerLazySingleton(() => Login(getIt<AuthRepository>()));
  getIt.registerLazySingleton(() => Logout(getIt<AuthRepository>()));
  getIt.registerLazySingleton(() => GetCurrentUser(getIt<AuthRepository>()));

  // BLoCs (register as factory -- new instance each time)
  getIt.registerFactory(
    () => AuthBloc(login: getIt<Login>()),
  );
}
```

### With injectable (Code Generation)

```yaml
# pubspec.yaml
dependencies:
  get_it: ^8.0.0
  injectable: ^2.5.0
dev_dependencies:
  injectable_generator: ^2.7.0
  build_runner: ^2.4.0
```

```dart
// lib/injection_container.dart
import 'package:get_it/get_it.dart';
import 'package:injectable/injectable.dart';
import 'injection_container.config.dart';

final getIt = GetIt.instance;

@InjectableInit()
Future<void> configureDependencies() async => getIt.init();

// Annotate classes for auto-registration
@lazySingleton
class AuthRemoteDataSourceImpl implements AuthRemoteDataSource {
  final Dio dio;
  AuthRemoteDataSourceImpl(this.dio);
}

@LazySingleton(as: AuthRepository)
class AuthRepositoryImpl implements AuthRepository {
  // ...
}

@injectable
class AuthBloc extends Bloc<AuthEvent, AuthState> {
  // ...
}

// External dependencies (SharedPreferences, Dio)
@module
abstract class RegisterModule {
  @preResolve
  @singleton
  Future<SharedPreferences> get prefs => SharedPreferences.getInstance();

  @singleton
  Dio get dio => Dio(BaseOptions(baseUrl: ApiConstants.baseUrl));
}
```

Run code generation:
```bash
dart run build_runner build --delete-conflicting-outputs
```

---

## 7. Mapping Between Entity and Model

```dart
// Extension method approach
extension UserModelMapper on UserModel {
  User toEntity() => User(
    id: id,
    name: name,
    email: email,
    role: role,
  );
}

extension UserMapper on User {
  UserModel toModel() => UserModel(
    id: id,
    name: name,
    email: email,
    role: role,
  );
}

// Or use inheritance (Model extends Entity)
class UserModel extends User {
  const UserModel({
    required super.id,
    required super.name,
    required super.email,
    required super.role,
  });

  factory UserModel.fromJson(Map<String, dynamic> json) { /* ... */ }
  Map<String, dynamic> toJson() { /* ... */ }
}
// UserModel IS a User, no mapping needed
```

---

## 8. Integration Testing Across Layers

```dart
void main() {
  late AuthRepository repository;
  late MockAuthRemoteDataSource mockRemote;
  late MockAuthLocalDataSource mockLocal;
  late MockNetworkInfo mockNetwork;

  setUp(() {
    mockRemote = MockAuthRemoteDataSource();
    mockLocal = MockAuthLocalDataSource();
    mockNetwork = MockNetworkInfo();
    repository = AuthRepositoryImpl(
      remoteDataSource: mockRemote,
      localDataSource: mockLocal,
      networkInfo: mockNetwork,
    );
  });

  group('login', () {
    test('returns User when remote call succeeds and device is online', () async {
      // Arrange
      when(() => mockNetwork.isConnected).thenAnswer((_) async => true);
      when(() => mockRemote.login(
        email: any(named: 'email'),
        password: any(named: 'password'),
      )).thenAnswer((_) async => tUserModel);
      when(() => mockLocal.cacheUser(any())).thenAnswer((_) async {});

      // Act
      final result = await repository.login(email: 'test@t.com', password: 'pw');

      // Assert
      expect(result, Right(tUserModel));
      verify(() => mockLocal.cacheUser(tUserModel)).called(1);
    });

    test('returns NetworkFailure when device is offline', () async {
      when(() => mockNetwork.isConnected).thenAnswer((_) async => false);

      final result = await repository.login(email: 'test@t.com', password: 'pw');

      expect(result, isA<Left>());
      result.fold(
        (failure) => expect(failure, isA<NetworkFailure>()),
        (_) => fail('Expected Left'),
      );
    });
  });
}
```

---

## 9. Use Case Testing

```dart
void main() {
  late Login loginUseCase;
  late MockAuthRepository mockRepository;

  setUp(() {
    mockRepository = MockAuthRepository();
    loginUseCase = Login(mockRepository);
  });

  test('should get user from repository on successful login', () async {
    // Arrange
    final tUser = User(id: '1', name: 'Test', email: 'test@t.com', role: UserRole.staff);
    when(() => mockRepository.login(
      email: any(named: 'email'),
      password: any(named: 'password'),
    )).thenAnswer((_) async => Right(tUser));

    // Act
    final result = await loginUseCase(
      const LoginParams(email: 'test@t.com', password: '123456'),
    );

    // Assert
    expect(result, Right(tUser));
    verify(() => mockRepository.login(email: 'test@t.com', password: '123456')).called(1);
    verifyNoMoreInteractions(mockRepository);
  });
}
```

---

## 10. When to Break the Rules (Pragmatic Shortcuts)

### Small features that don't need full clean architecture:

```
Skip Use Cases when:
  - The use case just passes through to the repository with no extra logic
  - Feature is a simple CRUD with no business rules
  - You're prototyping and will refactor later

Skip Repository abstraction when:
  - You have only one data source and won't change it
  - Feature is internal/admin-only with low change likelihood

Skip Entity/Model separation when:
  - Entity and Model are identical (no transformation needed)
  - Feature has no offline/caching requirement

The pragmatic approach:
  1. Start simple (no clean architecture)
  2. Extract use cases when business logic grows
  3. Add repository interface when you need a second data source
  4. Separate Entity/Model when serialization diverges from domain
  5. Full clean architecture for core business features
```

### Quick Feature (Pragmatic Structure)

```
lib/
├── features/
│   ├── settings/                    # Simple feature -- flat structure
│   │   ├── settings_page.dart
│   │   ├── settings_cubit.dart
│   │   └── settings_repository.dart # Concrete, no interface
│   │
│   ├── auth/                        # Complex feature -- full clean arch
│   │   ├── domain/
│   │   ├── data/
│   │   └── presentation/
```

### Decision Matrix

```
Feature complexity    Architecture level
-----------------    ------------------
Settings toggle      -> Cubit + SharedPreferences directly
User profile CRUD    -> Cubit + Repository (no use cases)
Auth with tokens     -> Full clean architecture
Payment processing   -> Full clean architecture + extra validation layer
Multi-tenant system  -> Full clean architecture + module boundaries
```
