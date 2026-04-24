# Flutter Debugging Patterns Reference

## 1. DevTools Deep Dive

### Performance View
```bash
# Launch in profile mode (NEVER debug mode for perf)
flutter run --profile
```

**Frame Analysis:**
- Green bar = frame rendered within budget (16ms for 60fps)
- Red bar = janky frame (exceeds budget)
- Click any bar to see detailed breakdown

**Widget Rebuild Tracking:**
1. Open DevTools > Performance
2. Enable "Track Widget Rebuilds"
3. Widgets that flash = unnecessary rebuilds
4. Fix: extract to `const` widgets, use `select` with Riverpod

### Memory View
```dart
// Take a memory snapshot programmatically
import 'dart:developer';

void debugMemory() {
  // Force GC and inspect
  developer.log('Memory snapshot', name: 'debug.memory');
}
```

**Leak Detection Pattern:**
1. Navigate to a screen
2. Take heap snapshot (DevTools > Memory > Snapshot)
3. Navigate away
4. Take another snapshot
5. Compare: objects that should be gone but aren't = leak

### Network View
- Shows all HTTP requests with timing, status, headers, body
- Filter by status code to find failures
- Check response time for performance bottlenecks

## 2. Advanced Programmatic Debugging

### Conditional Logging
```dart
import 'dart:developer' as dev;
import 'package:flutter/foundation.dart';

// Only log in debug mode
void debugLog(String message, {String name = 'app'}) {
  if (kDebugMode) {
    dev.log(message, name: name, time: DateTime.now());
  }
}

// Log with structured data (appears in DevTools Logging view)
void logEvent(String event, Map<String, dynamic> data) {
  dev.log(
    event,
    name: 'analytics',
    error: jsonEncode(data), // DevTools shows as expandable JSON
  );
}

// Performance timing
void timeOperation(String name, VoidCallback operation) {
  final stopwatch = Stopwatch()..start();
  operation();
  stopwatch.stop();
  dev.log('$name: ${stopwatch.elapsedMilliseconds}ms', name: 'perf');
}
```

### Widget Tree Inspection
```dart
// Dump entire widget tree to console
debugDumpApp();

// Dump render tree with sizes and constraints
debugDumpRenderTree();

// Dump layer tree (compositing layers)
debugDumpLayerTree();

// Dump focus tree (keyboard navigation)
debugDumpFocusTree();

// Dump semantics tree (accessibility)
debugDumpSemanticsTree(DebugSemanticsDumpOrder.inverseHitTest);
```

### Custom DiagnosticsNode
```dart
class OrderCard extends StatelessWidget {
  final Order order;
  const OrderCard({super.key, required this.order});

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(StringProperty('orderId', order.id));
    properties.add(DoubleProperty('total', order.total, unit: 'USD'));
    properties.add(IntProperty('items', order.items.length));
    properties.add(EnumProperty('status', order.status));
    properties.add(FlagProperty('isPaid',
      value: order.isPaid, ifTrue: 'PAID', ifFalse: 'UNPAID'));
    properties.add(ObjectFlagProperty('delivery',
      order.deliveryAddress, ifNull: 'no delivery address'));
  }

  @override
  Widget build(BuildContext context) => Card(child: Text(order.id));
}
```

## 3. Visual Debugging Flags Reference

```dart
import 'package:flutter/rendering.dart';

// === LAYOUT DEBUGGING ===
debugPaintSizeEnabled = true;          // Blue borders on all widgets
debugPaintBaselinesEnabled = true;     // Green/yellow baselines on text
debugPaintPointersEnabled = true;      // Teal circles at touch points
debugPaintLayerBordersEnabled = true;  // Orange borders on layers

// === REPAINT DEBUGGING ===
debugRepaintRainbowEnabled = true;     // Rainbow on repainted areas
debugRepaintTextRainbowEnabled = true; // Rainbow on text repaints

// === PERFORMANCE OVERLAY ===
// In MaterialApp:
MaterialApp(
  showPerformanceOverlay: true,          // Frame timing graph
  checkerboardRasterCacheImages: true,   // Checkerboard cached images
  checkerboardOffscreenLayers: true,     // Checkerboard offscreen layers
  showSemanticsDebugger: true,           // Accessibility overlay
)

// === FRAME TIMING ===
debugPrintBeginFrameBanner = true;     // "BUILD FRAME" markers
debugPrintEndFrameBanner = true;       // End frame markers
debugPrintScheduleFrameStacks = true;  // Why frame was scheduled
```

## 4. Error Handling Patterns

### Async Error Guard
```dart
// Pattern: Guard async callbacks against disposed state
class _MyScreenState extends State<MyScreen> {
  bool _isLoading = false;
  String? _error;

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    try {
      final data = await api.fetchData();
      if (!mounted) return; // CRITICAL: check before setState
      setState(() {
        _data = data;
        _isLoading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _isLoading = false;
      });
    }
  }
}
```

### Global Error Handler
```dart
void main() {
  // Catch Flutter framework errors
  FlutterError.onError = (FlutterErrorDetails details) {
    FlutterError.presentError(details);
    // Log to crash reporting service
    crashReporter.recordFlutterError(details);
  };

  // Catch async errors not handled by Flutter
  PlatformDispatcher.instance.onError = (error, stack) {
    crashReporter.recordError(error, stack);
    return true; // Handled
  };

  runApp(const MyApp());
}
```

### Error Boundary Widget
```dart
class ErrorBoundary extends StatefulWidget {
  final Widget child;
  const ErrorBoundary({super.key, required this.child});

  @override
  State<ErrorBoundary> createState() => _ErrorBoundaryState();
}

class _ErrorBoundaryState extends State<ErrorBoundary> {
  Object? _error;
  StackTrace? _stackTrace;

  @override
  void initState() {
    super.initState();
    FlutterError.onError = (details) {
      setState(() {
        _error = details.exception;
        _stackTrace = details.stack;
      });
    };
  }

  @override
  Widget build(BuildContext context) {
    if (_error != null) {
      return MaterialApp(
        home: Scaffold(
          body: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.error_outline, size: 48, color: Colors.red),
                const SizedBox(height: 16),
                Text('Something went wrong', style: Theme.of(context).textTheme.headlineMedium),
                const SizedBox(height: 8),
                if (kDebugMode) Text(_error.toString()),
                TextButton(
                  onPressed: () => setState(() { _error = null; _stackTrace = null; }),
                  child: const Text('Try Again'),
                ),
              ],
            ),
          ),
        ),
      );
    }
    return widget.child;
  }
}
```

## 5. Platform-Specific Debugging

### Android
```bash
# View Flutter logs only
adb logcat -s flutter

# View all logs with grep
adb logcat | grep -i "flutter\|dart"

# Clear logcat buffer
adb logcat -c

# Capture bug report
adb bugreport > bugreport.zip

# Check GPU rendering
adb shell dumpsys gfxinfo <package_name>
```

### iOS
```bash
# View device logs
idevicesyslog | grep -i flutter

# Or use Console.app
open /Applications/Utilities/Console.app

# Check for crashes
ls ~/Library/Logs/DiagnosticReports/
```

### Web
```dart
// Web-specific debugging
import 'package:flutter/foundation.dart' show kIsWeb;

if (kIsWeb) {
  // Use dart:html for web debugging
  // import 'dart:html' as html;
  // html.window.console.log('Web debug message');
}
```

## 6. Riverpod Debugging

```dart
// Custom ProviderObserver for logging all state changes
class LoggingObserver extends ProviderObserver {
  @override
  void didUpdateProvider(
    ProviderBase provider,
    Object? previousValue,
    Object? newValue,
    ProviderContainer container,
  ) {
    developer.log(
      '${provider.name ?? provider.runtimeType}: $previousValue -> $newValue',
      name: 'riverpod',
    );
  }

  @override
  void didDisposeProvider(ProviderBase provider, ProviderContainer container) {
    developer.log('Disposed: ${provider.name ?? provider.runtimeType}', name: 'riverpod');
  }
}

// Usage in main.dart
void main() {
  runApp(
    ProviderScope(
      observers: [if (kDebugMode) LoggingObserver()],
      child: const MyApp(),
    ),
  );
}
```

## 7. BLoC Debugging

```dart
// Custom BlocObserver for logging all events and state changes
class LoggingBlocObserver extends BlocObserver {
  @override
  void onEvent(Bloc bloc, Object? event) {
    super.onEvent(bloc, event);
    developer.log('${bloc.runtimeType} Event: $event', name: 'bloc');
  }

  @override
  void onChange(BlocBase bloc, Change change) {
    super.onChange(bloc, change);
    developer.log(
      '${bloc.runtimeType}: ${change.currentState} -> ${change.nextState}',
      name: 'bloc',
    );
  }

  @override
  void onError(BlocBase bloc, Object error, StackTrace stackTrace) {
    super.onError(bloc, error, stackTrace);
    developer.log(
      '${bloc.runtimeType} Error: $error',
      name: 'bloc.error',
      error: error,
      stackTrace: stackTrace,
    );
  }
}

// Usage in main.dart
void main() {
  Bloc.observer = LoggingBlocObserver();
  runApp(const MyApp());
}
```

## 8. Common Debug Scenarios

### "Widget not rebuilding"
1. Check if using `ref.watch()` (not `ref.read()`) for reactive updates
2. Check if provider is `autoDispose` and was already disposed
3. Check if the state object implements proper equality (use `freezed`)
4. Use `debugPrint` in `build()` to verify rebuilds

### "Layout overflow"
1. Enable `debugPaintSizeEnabled = true`
2. Check constraints with `debugDumpRenderTree()`
3. Common fix: wrap overflowing child in `Expanded`, `Flexible`, or `SingleChildScrollView`

### "Slow scrolling"
1. Profile in `--profile` mode (NOT debug)
2. Check for expensive `build()` in list items
3. Ensure using `ListView.builder` (not `ListView` with all children)
4. Add `itemExtent` or `prototypeItem` for fixed-height items
5. Add `RepaintBoundary` around complex list items
6. Use `const` constructors everywhere possible

### "App crash on startup"
1. `flutter clean && flutter pub get`
2. `flutter doctor -v` to check environment
3. Check `android/app/build.gradle` for minSdk/targetSdk issues
4. Check `ios/Podfile` for platform version
5. Check for missing permissions in manifests
