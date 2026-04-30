# Riverpod 3.0 Complete Patterns Reference

> Comprehensive Riverpod patterns for Flutter applications.
> Covers all provider types, code generation, families, lifecycle, testing, and real-world patterns.

---

## 1. All Provider Types

### Provider — Computed/Static Values

```dart
// Simple computed value
final appNameProvider = Provider<String>((ref) => 'My App');

// Computed from other providers
final filteredTodosProvider = Provider<List<Todo>>((ref) {
  final todos = ref.watch(todosProvider);
  final filter = ref.watch(filterProvider);

  return switch (filter) {
    TodoFilter.all       => todos,
    TodoFilter.completed => todos.where((t) => t.isCompleted).toList(),
    TodoFilter.active    => todos.where((t) => !t.isCompleted).toList(),
  };
});
```

### StateProvider — Simple Mutable State

```dart
final counterProvider = StateProvider<int>((ref) => 0);
final filterProvider = StateProvider<TodoFilter>((ref) => TodoFilter.all);

// In widget:
ref.read(counterProvider.notifier).state++;
ref.read(filterProvider.notifier).state = TodoFilter.completed;

// Or update based on previous value:
ref.read(counterProvider.notifier).update((state) => state + 1);
```

### FutureProvider — One-Shot Async Data

```dart
final userProvider = FutureProvider<User>((ref) async {
  final client = ref.watch(httpClientProvider);
  final response = await client.get('/api/user');
  return User.fromJson(response.data);
});

// Auto-refresh when dependency changes
final productProvider = FutureProvider.family<Product, String>((ref, id) async {
  final client = ref.watch(httpClientProvider);
  return Product.fromJson((await client.get('/api/products/$id')).data);
});

// In widget:
ref.watch(userProvider).when(
  data: (user) => Text(user.name),
  loading: () => const CircularProgressIndicator(),
  error: (err, stack) => ErrorWidget(err.toString()),
);
```

### StreamProvider — Real-Time Data

```dart
final messagesProvider = StreamProvider<List<Message>>((ref) {
  final firestore = ref.watch(firestoreProvider);
  return firestore
      .collection('messages')
      .orderBy('timestamp', descending: true)
      .limit(50)
      .snapshots()
      .map((snap) => snap.docs.map(Message.fromFirestore).toList());
});

final connectivityProvider = StreamProvider<ConnectivityResult>((ref) {
  return Connectivity().onConnectivityChanged;
});
```

### NotifierProvider — Complex Synchronous State

```dart
final cartProvider = NotifierProvider<CartNotifier, CartState>(CartNotifier.new);

class CartNotifier extends Notifier<CartState> {
  @override
  CartState build() => const CartState(items: [], total: 0);

  void addItem(Product product, {int quantity = 1}) {
    final existingIndex = state.items.indexWhere((i) => i.product.id == product.id);
    final updatedItems = [...state.items];

    if (existingIndex >= 0) {
      final existing = updatedItems[existingIndex];
      updatedItems[existingIndex] = existing.copyWith(
        quantity: existing.quantity + quantity,
      );
    } else {
      updatedItems.add(CartItem(product: product, quantity: quantity));
    }

    state = state.copyWith(
      items: updatedItems,
      total: _calculateTotal(updatedItems),
    );
  }

  void removeItem(String productId) {
    final updatedItems = state.items.where((i) => i.product.id != productId).toList();
    state = state.copyWith(
      items: updatedItems,
      total: _calculateTotal(updatedItems),
    );
  }

  void clearCart() {
    state = const CartState(items: [], total: 0);
  }

  double _calculateTotal(List<CartItem> items) {
    return items.fold(0, (sum, item) => sum + item.product.price * item.quantity);
  }
}
```

### AsyncNotifierProvider — Complex Async State

```dart
final todosProvider =
    AsyncNotifierProvider<TodosNotifier, List<Todo>>(TodosNotifier.new);

class TodosNotifier extends AsyncNotifier<List<Todo>> {
  @override
  Future<List<Todo>> build() async {
    // This runs on first access and when dependencies change
    final repo = ref.watch(todoRepositoryProvider);
    return repo.fetchAll();
  }

  Future<void> addTodo(String title) async {
    final repo = ref.read(todoRepositoryProvider);
    // Optimistic update
    final newTodo = Todo(id: UniqueKey().toString(), title: title);
    state = AsyncData([...state.requireValue, newTodo]);

    try {
      await repo.add(newTodo);
    } catch (e, st) {
      // Revert on failure
      state = AsyncData(state.requireValue.where((t) => t.id != newTodo.id).toList());
      state = AsyncError(e, st);
    }
  }

  Future<void> toggleTodo(String id) async {
    state = AsyncData(
      state.requireValue.map((todo) {
        return todo.id == id ? todo.copyWith(isCompleted: !todo.isCompleted) : todo;
      }).toList(),
    );
    final repo = ref.read(todoRepositoryProvider);
    await repo.toggle(id);
  }

  Future<void> deleteTodo(String id) async {
    final backup = state.requireValue;
    state = AsyncData(backup.where((t) => t.id != id).toList());

    try {
      await ref.read(todoRepositoryProvider).delete(id);
    } catch (e) {
      state = AsyncData(backup); // Revert
      rethrow;
    }
  }
}
```

---

## 2. Code Generation with @riverpod

```dart
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'providers.g.dart';

// Equivalent to Provider<String>
@riverpod
String greeting(GreetingRef ref) => 'Hello from Riverpod!';

// Equivalent to FutureProvider<User>
@riverpod
Future<User> currentUser(CurrentUserRef ref) async {
  final repo = ref.watch(authRepositoryProvider);
  return repo.getCurrentUser();
}

// Equivalent to NotifierProvider
@riverpod
class Counter extends _$Counter {
  @override
  int build() => 0;

  void increment() => state++;
  void decrement() => state--;
}

// Equivalent to AsyncNotifierProvider
@riverpod
class Todos extends _$Todos {
  @override
  Future<List<Todo>> build() async {
    return ref.watch(todoRepositoryProvider).fetchAll();
  }

  Future<void> add(Todo todo) async {
    await ref.read(todoRepositoryProvider).add(todo);
    ref.invalidateSelf(); // Re-fetch
    await future; // Wait for rebuild
  }
}

// Family (parameterized) — automatically generated
@riverpod
Future<Product> product(ProductRef ref, String id) async {
  return ref.watch(productRepositoryProvider).getById(id);
}

// Usage: ref.watch(productProvider('product-123'))
```

Run code generation:
```bash
dart run build_runner build --delete-conflicting-outputs
# Or watch mode during development:
dart run build_runner watch --delete-conflicting-outputs
```

---

## 3. Family Providers (Parameterized)

```dart
// Without code generation
final productProvider = FutureProvider.family<Product, String>((ref, productId) async {
  final repo = ref.watch(productRepositoryProvider);
  return repo.getById(productId);
});

// Multiple parameters — use a record
final searchProvider = FutureProvider.family<List<Product>, ({String query, int page})>(
  (ref, params) async {
    final repo = ref.watch(productRepositoryProvider);
    return repo.search(query: params.query, page: params.page);
  },
);

// Usage:
ref.watch(searchProvider((query: 'shoes', page: 1)));
```

---

## 4. Provider Lifecycle

### Auto-Dispose (Default with Code Gen)

```dart
// Manual: add .autoDispose
final userProvider = FutureProvider.autoDispose<User>((ref) async {
  // Provider is disposed when no widget watches it
  ref.onDispose(() {
    print('User provider disposed');
  });
  return fetchUser();
});

// Keep alive temporarily
@riverpod
Future<User> user(UserRef ref) async {
  // Keep alive for 30 seconds after last listener removed
  final link = ref.keepAlive();
  final timer = Timer(const Duration(seconds: 30), link.close);
  ref.onDispose(timer.cancel);

  return fetchUser();
}
```

### Refresh and Invalidate

```dart
// Force re-fetch (invalidate + wait for new value)
await ref.refresh(todosProvider.future);

// Invalidate (marks as stale, re-fetches on next watch)
ref.invalidate(todosProvider);

// Self-invalidation inside a notifier
ref.invalidateSelf();
```

---

## 5. Combining Providers

```dart
// Provider that depends on other providers
final userTodosProvider = FutureProvider<List<Todo>>((ref) async {
  final user = await ref.watch(currentUserProvider.future);
  final todos = await ref.watch(todosProvider.future);
  return todos.where((t) => t.userId == user.id).toList();
});

// Selective watching (only rebuild when specific field changes)
final userNameProvider = Provider<String>((ref) {
  return ref.watch(userProvider.select((user) => user.name));
});
```

---

## 6. Testing Providers

```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

// Unit testing a provider
test('todosProvider returns list of todos', () async {
  final container = ProviderContainer(
    overrides: [
      todoRepositoryProvider.overrideWithValue(MockTodoRepository()),
    ],
  );
  addTearDown(container.dispose);

  // Wait for async provider
  final todos = await container.read(todosProvider.future);
  expect(todos, hasLength(3));
});

// Testing a notifier
test('CartNotifier adds item correctly', () {
  final container = ProviderContainer();
  addTearDown(container.dispose);

  final notifier = container.read(cartProvider.notifier);
  final product = Product(id: '1', name: 'Test', price: 9.99);

  notifier.addItem(product);

  final state = container.read(cartProvider);
  expect(state.items, hasLength(1));
  expect(state.total, 9.99);
});

// Widget testing with provider overrides
testWidgets('Home screen shows user name', (tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        currentUserProvider.overrideWith(
          (ref) async => User(id: '1', name: 'the user'),
        ),
      ],
      child: const MaterialApp(home: HomeScreen()),
    ),
  );

  await tester.pumpAndSettle();
  expect(find.text('Hello, the user'), findsOneWidget);
});
```

---

## 7. Real-World Patterns

### Pagination

```dart
@riverpod
class ProductList extends _$ProductList {
  int _page = 0;
  bool _hasMore = true;

  @override
  Future<List<Product>> build() async {
    _page = 0;
    _hasMore = true;
    return _fetch(0);
  }

  Future<List<Product>> _fetch(int page) async {
    final repo = ref.read(productRepositoryProvider);
    final products = await repo.getProducts(page: page, limit: 20);
    _hasMore = products.length == 20;
    return products;
  }

  Future<void> loadMore() async {
    if (!_hasMore || state is AsyncLoading) return;
    _page++;
    final newProducts = await _fetch(_page);
    state = AsyncData([...state.requireValue, ...newProducts]);
  }

  bool get hasMore => _hasMore;
}
```

### Search-As-You-Type (Debounced)

```dart
final searchQueryProvider = StateProvider<String>((ref) => '');

final searchResultsProvider = FutureProvider<List<Product>>((ref) async {
  final query = ref.watch(searchQueryProvider);
  if (query.isEmpty) return [];

  // Debounce: cancel if query changes within 500ms
  await Future<void>.delayed(const Duration(milliseconds: 500));
  if (ref.read(searchQueryProvider) != query) {
    throw Exception('Cancelled');
  }

  return ref.read(productRepositoryProvider).search(query);
});
```

### Auth Guard Pattern

```dart
final authStateProvider = StreamProvider<User?>((ref) {
  return ref.watch(authRepositoryProvider).authStateChanges;
});

// In GoRouter redirect:
redirect: (context, state) {
  final authState = ref.read(authStateProvider);
  final isAuthenticated = authState.valueOrNull != null;
  final isAuthRoute = state.matchedLocation.startsWith('/auth');

  if (!isAuthenticated && !isAuthRoute) return '/auth/login';
  if (isAuthenticated && isAuthRoute) return '/';
  return null;
},
```

---

## 8. Migration from Provider to Riverpod

| Provider | Riverpod |
|----------|----------|
| `ChangeNotifierProvider` | `NotifierProvider` |
| `FutureProvider` | `FutureProvider` (same name!) |
| `StreamProvider` | `StreamProvider` (same name!) |
| `Provider` | `Provider` (same name!) |
| `context.read<T>()` | `ref.read(provider)` |
| `context.watch<T>()` | `ref.watch(provider)` |
| `Consumer` widget | `ConsumerWidget` or `Consumer` |
| `MultiProvider` | `ProviderScope` |
| `ProxyProvider` | Provider that `ref.watch`es another |

Key differences:
- Riverpod does NOT depend on BuildContext (can be used anywhere)
- Riverpod has compile-time safety (no runtime ProviderNotFoundException)
- Riverpod providers are global declarations (not widget-tree scoped)
- Riverpod supports auto-dispose out of the box
- Riverpod has built-in code generation for less boilerplate
