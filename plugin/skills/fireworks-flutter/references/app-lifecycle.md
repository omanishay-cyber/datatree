# App Lifecycle and State Restoration Reference

> AppLifecycleListener, RestorationMixin, restorationScopeId patterns for Flutter 3.38+.

---

## 1. AppLifecycleListener (Replaces WidgetsBindingObserver)

```dart
class _AppShellState extends State<AppShell> {
  late final AppLifecycleListener _lifecycleListener;

  @override
  void initState() {
    super.initState();
    _lifecycleListener = AppLifecycleListener(
      onResume: _onResume,
      onInactive: _onInactive,
      onHide: _onHide,
      onPause: _onPause,
      onDetach: _onDetach,
      onRestart: _onRestart,
      onExitRequested: _onExitRequested,
      onStateChange: _onStateChange,
    );
  }

  void _onResume() {
    // App is visible and responding to input
    // Refresh data, reconnect WebSocket, resume animations
    ref.invalidate(dashboardProvider);
  }

  void _onInactive() {
    // App is visible but not responding to input (phone call overlay, etc.)
    // Pause non-essential animations
  }

  void _onHide() {
    // App UI is no longer visible
    // Save draft state, pause heavy computations
  }

  void _onPause() {
    // App may be suspended soon
    // Persist critical state to disk
    ref.read(settingsProvider.notifier).persistToDisk();
  }

  void _onDetach() {
    // App is still hosted but detached from views
    // Rare -- mostly for multi-window scenarios
  }

  void _onRestart() {
    // App was paused and is now resumed
    // Re-initialize platform channels if needed
  }

  Future<AppExitResponse> _onExitRequested() async {
    // Desktop only -- user requested app exit
    final hasUnsavedChanges = ref.read(formStateProvider).isDirty;
    if (hasUnsavedChanges) {
      final shouldExit = await _showSaveDialog();
      return shouldExit ? AppExitResponse.exit : AppExitResponse.cancel;
    }
    return AppExitResponse.exit;
  }

  void _onStateChange(AppLifecycleState state) {
    debugPrint('Lifecycle state: $state');
  }

  @override
  void dispose() {
    _lifecycleListener.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
```

### Lifecycle State Flow

```
                  +-----------+
                  |  detached |
                  +-----+-----+
                        |
                        v
                  +-----------+
           +----->|  resumed  |<-----+
           |      +-----+-----+     |
           |            |            |
           |            v            |
           |      +-----------+     |
           |      |  inactive |     |
           |      +-----+-----+     |
           |            |            |
           |            v            |
           |      +-----------+     |
           +------+   hidden  +-----+
                  +-----+-----+
                        |
                        v
                  +-----------+
                  |   paused  |
                  +-----------+
```

---

## 2. RestorationMixin for State Preservation

Restore widget state across process kills (Android low-memory, iOS background termination).

```dart
class _CounterPageState extends State<CounterPage> with RestorationMixin {
  // Declare restorable properties
  final RestorableInt _counter = RestorableInt(0);
  final RestorableString _lastAction = RestorableString('none');
  final RestorableBool _isExpanded = RestorableBool(false);

  @override
  String? get restorationId => 'counter_page';

  @override
  void restoreState(RestorationBucket? oldBucket, bool initialRestore) {
    // Register each restorable property
    registerForRestoration(_counter, 'counter');
    registerForRestoration(_lastAction, 'last_action');
    registerForRestoration(_isExpanded, 'is_expanded');
  }

  @override
  void dispose() {
    _counter.dispose();
    _lastAction.dispose();
    _isExpanded.dispose();
    super.dispose();
  }

  void _increment() {
    setState(() {
      _counter.value++;
      _lastAction.value = 'increment';
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(child: Text('Count: ${_counter.value}')),
      floatingActionButton: FloatingActionButton(
        onPressed: _increment,
        child: const Icon(Icons.add),
      ),
    );
  }
}
```

### Restorable Types

| Type | Usage |
|---|---|
| `RestorableInt` | Integer values (counters, indices) |
| `RestorableDouble` | Double values (scroll position, slider) |
| `RestorableString` | Text values (search query, draft) |
| `RestorableBool` | Boolean flags (expanded, selected) |
| `RestorableNum<T>` | Generic numeric |
| `RestorableDateTime` | Date/time values |
| `RestorableTextEditingController` | Text field state |
| `RestorableEnum<T>` | Enum values |
| `RestorableValue<T>` | Custom restorable (implement yourself) |

### Custom RestorableValue

```dart
class RestorableUser extends RestorableValue<User?> {
  @override
  User? createDefaultValue() => null;

  @override
  void didUpdateValue(User? oldValue) {
    notifyListeners();
  }

  @override
  User? fromPrimitives(Object? data) {
    if (data == null) return null;
    final map = Map<String, dynamic>.from(data as Map);
    return User.fromJson(map);
  }

  @override
  Object? toPrimitives() => value?.toJson();
}
```

---

## 3. restorationScopeId on MaterialApp

```dart
MaterialApp.router(
  restorationScopeId: 'app',  // Enable restoration for entire app
  routerConfig: router,
)

// On individual routes
GoRoute(
  path: '/form',
  builder: (context, state) => const FormPage(),
  restorationScopeId: 'form_page',  // GoRouter does not have this directly
)
```

### Restoration with GoRouter

GoRouter preserves navigation state automatically. For widget-level restoration, use `RestorationMixin` on individual page states.

```dart
// Restore scroll position
class _ProductListState extends State<ProductList> with RestorationMixin {
  final RestorableDouble _scrollOffset = RestorableDouble(0);
  late final ScrollController _scrollController;

  @override
  String? get restorationId => 'product_list';

  @override
  void restoreState(RestorationBucket? oldBucket, bool initialRestore) {
    registerForRestoration(_scrollOffset, 'scroll_offset');
    _scrollController = ScrollController(initialScrollOffset: _scrollOffset.value);
    _scrollController.addListener(() {
      _scrollOffset.value = _scrollController.offset;
    });
  }

  @override
  void dispose() {
    _scrollOffset.dispose();
    _scrollController.dispose();
    super.dispose();
  }
}
```

---

## 4. Platform-Specific Lifecycle Notes

| Platform | Behavior |
|---|---|
| **Android** | Process may be killed when backgrounded; restoration is critical |
| **iOS** | Scene lifecycle (3.38+); suspended apps stay in memory longer |
| **Web** | No process kill; use `onHide` for tab visibility changes |
| **Desktop** | `onExitRequested` allows preventing close; restoration less critical |

### Testing Lifecycle

```bash
# Android: simulate process kill
adb shell am kill com.example.app
# Then reopen the app -- restoration should work

# iOS: use Xcode > Debug > Simulate Memory Warning

# Flutter: test restoration
flutter run --restore-state-from-file=/path/to/state.bin
```
