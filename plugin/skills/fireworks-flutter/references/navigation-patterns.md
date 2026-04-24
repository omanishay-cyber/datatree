# Navigation Patterns Reference (GoRouter + Flutter 3.38)

> GoRouter with StatefulShellRoute, TypedQueryParameter, deep linking, and page transitions.

---

## 1. GoRouter Setup with Riverpod

```dart
import 'package:go_router/go_router.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'router.g.dart';

@riverpod
GoRouter router(Ref ref) {
  final authState = ref.watch(authStateProvider);
  return GoRouter(
    initialLocation: '/',
    debugLogDiagnostics: true,
    redirect: (context, state) {
      final isAuthenticated = authState.valueOrNull != null;
      final isAuthRoute = state.matchedLocation.startsWith('/auth');
      if (!isAuthenticated && !isAuthRoute) return '/auth/login';
      if (isAuthenticated && isAuthRoute) return '/';
      return null;
    },
    routes: $appRoutes,
    errorBuilder: (context, state) => ErrorScreen(error: state.error),
  );
}

class MyApp extends ConsumerWidget {
  const MyApp({super.key});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    return MaterialApp.router(routerConfig: router);
  }
}
```

---

## 2. StatefulShellRoute (Persistent Tab Navigation)

Preserves navigation state of each tab. When switching tabs, previous tab state is maintained.

```dart
StatefulShellRoute.indexedStack(
  builder: (context, state, navigationShell) {
    return ScaffoldWithNavBar(navigationShell: navigationShell);
  },
  branches: [
    StatefulShellBranch(routes: [
      GoRoute(
        path: '/home',
        builder: (context, state) => const HomeScreen(),
        routes: [
          GoRoute(
            path: 'details/:id',
            builder: (context, state) =>
                DetailsScreen(id: state.pathParameters['id']!),
          ),
        ],
      ),
    ]),
    StatefulShellBranch(routes: [
      GoRoute(path: '/search', builder: (_, __) => const SearchScreen()),
    ]),
    StatefulShellBranch(routes: [
      GoRoute(path: '/orders', builder: (_, __) => const OrdersScreen()),
    ]),
    StatefulShellBranch(routes: [
      GoRoute(
        path: '/profile',
        builder: (_, __) => const ProfileScreen(),
        routes: [
          GoRoute(path: 'settings', builder: (_, __) => const SettingsScreen()),
        ],
      ),
    ]),
  ],
)
```

### ScaffoldWithNavBar

```dart
class ScaffoldWithNavBar extends StatelessWidget {
  final StatefulNavigationShell navigationShell;
  const ScaffoldWithNavBar({super.key, required this.navigationShell});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: navigationShell,
      bottomNavigationBar: NavigationBar(
        selectedIndex: navigationShell.currentIndex,
        onDestinationSelected: (index) {
          navigationShell.goBranch(
            index,
            initialLocation: index == navigationShell.currentIndex,
          );
        },
        destinations: const [
          NavigationDestination(icon: Icon(Icons.home), label: 'Home'),
          NavigationDestination(icon: Icon(Icons.search), label: 'Search'),
          NavigationDestination(icon: Icon(Icons.receipt), label: 'Orders'),
          NavigationDestination(icon: Icon(Icons.person), label: 'Profile'),
        ],
      ),
    );
  }
}
```

---

## 3. Type-Safe Routes (go_router_builder)

```dart
part 'routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/', routes: [
  TypedGoRoute<ProductRoute>(path: 'product/:id'),
  TypedGoRoute<SearchRoute>(path: 'search'),
])
class HomeRoute extends GoRouteData {
  const HomeRoute();
  @override
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class ProductRoute extends GoRouteData {
  final String id;
  const ProductRoute({required this.id});
  @override
  Widget build(BuildContext context, GoRouterState state) =>
      ProductScreen(productId: id);
}

class SearchRoute extends GoRouteData {
  final String? q;
  final int page;
  const SearchRoute({this.q, this.page = 1});
  @override
  Widget build(BuildContext context, GoRouterState state) =>
      SearchScreen(query: q ?? '', page: page);
}

// Type-safe navigation:
const HomeRoute().go(context);
ProductRoute(id: '123').push(context);
SearchRoute(q: 'shoes', page: 2).go(context);
```

---

## 4. Deep Linking

### Android (AndroidManifest.xml)

```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="https" android:host="myapp.example.com" />
</intent-filter>
```

### iOS (Info.plist)

```xml
<key>CFBundleURLSchemes</key>
<array><string>myapp</string></array>
<key>com.apple.developer.associated-domains</key>
<array><string>applinks:myapp.example.com</string></array>
```

---

## 5. Page Transitions

```dart
GoRoute(
  path: '/details/:id',
  pageBuilder: (context, state) => CustomTransitionPage(
    key: state.pageKey,
    child: DetailsScreen(id: state.pathParameters['id']!),
    transitionsBuilder: (context, animation, secondaryAnimation, child) {
      return SlideTransition(
        position: Tween<Offset>(begin: const Offset(1, 0), end: Offset.zero)
            .animate(CurvedAnimation(parent: animation, curve: Curves.easeOutCubic)),
        child: child,
      );
    },
  ),
)

// Fade transition
transitionsBuilder: (_, animation, __, child) =>
    FadeTransition(opacity: animation, child: child)

// No transition
transitionsBuilder: (_, __, ___, child) => child
```

---

## 6. AppLifecycleListener

```dart
class _MyAppState extends State<MyApp> {
  late final AppLifecycleListener _listener;

  @override
  void initState() {
    super.initState();
    _listener = AppLifecycleListener(
      onResume: () => debugPrint('resumed'),
      onInactive: () => debugPrint('inactive'),
      onHide: () => debugPrint('hidden'),
      onPause: () => debugPrint('paused'),
      onExitRequested: () async => AppExitResponse.exit,
    );
  }

  @override
  void dispose() {
    _listener.dispose();
    super.dispose();
  }
}
```

---

## 7. Quick Reference

| Pattern | Use When |
|---|---|
| `context.go('/path')` | Replace entire stack |
| `context.push('/path')` | Push onto stack |
| `context.pop()` | Go back |
| `context.pushReplacement('/path')` | Replace current route |
| `StatefulShellRoute.indexedStack` | Bottom nav with state |
| `ShellRoute` | Shared layout, no state preservation |
| `navigationShell.goBranch(i)` | Switch tabs |

### Common Mistakes

| Mistake | Fix |
|---|---|
| `push` for tab nav | Use `go` |
| Back button with StatefulShellRoute | Override `PopScope` |
| Deep link fails iOS | Check `apple-app-site-association` |
| Query params lost on redirect | Preserve in redirect URL |
| Router ignores auth changes | Use `ref.watch` not `ref.read` |
