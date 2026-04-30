# Essential Flutter Widget Patterns & Material 3 Reference

> Comprehensive widget catalog covering Material 3 theming, responsive layouts,
> lists, forms, dialogs, animations, custom painting, slivers, performance,
> accessibility, and common gotchas.

---

## 1. Material 3 Theming

### ColorScheme from Seed

```dart
MaterialApp(
  theme: ThemeData(
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(
      seedColor: const Color(0xFF6750A4),
      brightness: Brightness.light,
    ),
  ),
  darkTheme: ThemeData(
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(
      seedColor: const Color(0xFF6750A4),
      brightness: Brightness.dark,
    ),
  ),
  themeMode: ThemeMode.system,
)
```

### Dynamic Color (Android 12+)

```yaml
# pubspec.yaml
dependencies:
  dynamic_color: ^1.7.0
```

```dart
DynamicColorBuilder(
  builder: (ColorScheme? lightDynamic, ColorScheme? darkDynamic) {
    return MaterialApp(
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: lightDynamic ?? ColorScheme.fromSeed(
          seedColor: const Color(0xFF6750A4),
        ),
      ),
      darkTheme: ThemeData(
        useMaterial3: true,
        colorScheme: darkDynamic ?? ColorScheme.fromSeed(
          seedColor: const Color(0xFF6750A4),
          brightness: Brightness.dark,
        ),
      ),
    );
  },
)
```

### Custom Color Scheme

```dart
final colorScheme = ColorScheme.fromSeed(
  seedColor: Colors.blue,
).copyWith(
  // Override specific roles
  tertiary: const Color(0xFFE8DEF8),
  error: const Color(0xFFBA1A1A),
  surface: const Color(0xFFFFFBFE),
);
```

### Accessing Colors in Widgets

```dart
// Always use colorScheme, never hardcode colors
final scheme = Theme.of(context).colorScheme;

Container(
  color: scheme.primaryContainer,
  child: Text(
    'Hello',
    style: TextStyle(color: scheme.onPrimaryContainer),
  ),
)

// Color roles:
// primary / onPrimary / primaryContainer / onPrimaryContainer
// secondary / onSecondary / secondaryContainer / onSecondaryContainer
// tertiary / onTertiary / tertiaryContainer / onTertiaryContainer
// error / onError / errorContainer / onErrorContainer
// surface / onSurface / surfaceContainerHighest / outline
```

---

## 2. Typography -- Material 3 Type Scale

```dart
ThemeData(
  textTheme: const TextTheme(
    displayLarge:  TextStyle(fontSize: 57, fontWeight: FontWeight.w400),
    displayMedium: TextStyle(fontSize: 45, fontWeight: FontWeight.w400),
    displaySmall:  TextStyle(fontSize: 36, fontWeight: FontWeight.w400),
    headlineLarge: TextStyle(fontSize: 32, fontWeight: FontWeight.w400),
    headlineMedium:TextStyle(fontSize: 28, fontWeight: FontWeight.w400),
    headlineSmall: TextStyle(fontSize: 24, fontWeight: FontWeight.w400),
    titleLarge:    TextStyle(fontSize: 22, fontWeight: FontWeight.w400),
    titleMedium:   TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
    titleSmall:    TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
    bodyLarge:     TextStyle(fontSize: 16, fontWeight: FontWeight.w400),
    bodyMedium:    TextStyle(fontSize: 14, fontWeight: FontWeight.w400),
    bodySmall:     TextStyle(fontSize: 12, fontWeight: FontWeight.w400),
    labelLarge:    TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
    labelMedium:   TextStyle(fontSize: 12, fontWeight: FontWeight.w500),
    labelSmall:    TextStyle(fontSize: 11, fontWeight: FontWeight.w500),
  ),
)

// Usage
Text('Title', style: Theme.of(context).textTheme.headlineMedium)
Text('Body', style: Theme.of(context).textTheme.bodyLarge)
```

### Google Fonts Integration

```dart
import 'package:google_fonts/google_fonts.dart';

ThemeData(
  textTheme: GoogleFonts.interTextTheme(),
  // Or customize:
  textTheme: GoogleFonts.poppinsTextTheme().copyWith(
    headlineLarge: GoogleFonts.playfairDisplay(fontSize: 32),
  ),
)
```

---

## 3. Common Layouts

### Scaffold with Material 3 Navigation

```dart
// Bottom Navigation (NavigationBar -- Material 3)
Scaffold(
  body: _pages[_selectedIndex],
  bottomNavigationBar: NavigationBar(
    selectedIndex: _selectedIndex,
    onDestinationSelected: (index) => setState(() => _selectedIndex = index),
    destinations: const [
      NavigationDestination(icon: Icon(Icons.home), label: 'Home'),
      NavigationDestination(icon: Icon(Icons.search), label: 'Search'),
      NavigationDestination(icon: Icon(Icons.person), label: 'Profile'),
    ],
  ),
)

// Navigation Rail (for tablets/desktop)
Scaffold(
  body: Row(
    children: [
      NavigationRail(
        selectedIndex: _selectedIndex,
        onDestinationSelected: (index) => setState(() => _selectedIndex = index),
        labelType: NavigationRailLabelType.all,
        leading: FloatingActionButton(
          onPressed: () {},
          child: const Icon(Icons.add),
        ),
        destinations: const [
          NavigationRailDestination(icon: Icon(Icons.home), label: Text('Home')),
          NavigationRailDestination(icon: Icon(Icons.search), label: Text('Search')),
          NavigationRailDestination(icon: Icon(Icons.settings), label: Text('Settings')),
        ],
      ),
      const VerticalDivider(thickness: 1, width: 1),
      Expanded(child: _pages[_selectedIndex]),
    ],
  ),
)

// Navigation Drawer
Scaffold(
  drawer: NavigationDrawer(
    selectedIndex: _selectedIndex,
    onDestinationSelected: (index) {
      setState(() => _selectedIndex = index);
      Navigator.of(context).pop(); // Close drawer
    },
    children: [
      const Padding(
        padding: EdgeInsets.fromLTRB(28, 16, 16, 10),
        child: Text('App Name', style: TextStyle(fontSize: 18)),
      ),
      const NavigationDrawerDestination(
        icon: Icon(Icons.home_outlined),
        selectedIcon: Icon(Icons.home),
        label: Text('Home'),
      ),
      const NavigationDrawerDestination(
        icon: Icon(Icons.settings_outlined),
        selectedIcon: Icon(Icons.settings),
        label: Text('Settings'),
      ),
    ],
  ),
)
```

### AppBar Variants

```dart
// Standard
AppBar(title: const Text('Title'))

// Large (Material 3)
SliverAppBar.large(title: const Text('Large Title'))

// Medium
SliverAppBar.medium(title: const Text('Medium Title'))

// With search
AppBar(
  title: const Text('Products'),
  actions: [
    IconButton(
      icon: const Icon(Icons.search),
      onPressed: () => showSearch(context: context, delegate: ProductSearch()),
    ),
  ],
)

// SearchAnchor (Material 3)
SearchAnchor(
  builder: (context, controller) {
    return IconButton(
      icon: const Icon(Icons.search),
      onPressed: () => controller.openView(),
    );
  },
  suggestionsBuilder: (context, controller) {
    return List.generate(5, (index) {
      return ListTile(
        title: Text('Suggestion $index'),
        onTap: () => controller.closeView('Suggestion $index'),
      );
    });
  },
)
```

---

## 4. Responsive Layouts

### LayoutBuilder

```dart
LayoutBuilder(
  builder: (context, constraints) {
    if (constraints.maxWidth >= 1200) {
      return const DesktopLayout();
    } else if (constraints.maxWidth >= 600) {
      return const TabletLayout();
    } else {
      return const MobileLayout();
    }
  },
)
```

### Adaptive Breakpoints

```dart
enum ScreenSize { compact, medium, expanded, large, extraLarge }

ScreenSize getScreenSize(BuildContext context) {
  final width = MediaQuery.sizeOf(context).width;
  if (width < 600) return ScreenSize.compact;
  if (width < 840) return ScreenSize.medium;
  if (width < 1200) return ScreenSize.expanded;
  if (width < 1600) return ScreenSize.large;
  return ScreenSize.extraLarge;
}

// Responsive grid
GridView.builder(
  gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
    crossAxisCount: switch (getScreenSize(context)) {
      ScreenSize.compact => 1,
      ScreenSize.medium => 2,
      ScreenSize.expanded => 3,
      ScreenSize.large => 4,
      ScreenSize.extraLarge => 5,
    },
    crossAxisSpacing: 16,
    mainAxisSpacing: 16,
  ),
  itemCount: items.length,
  itemBuilder: (context, index) => ItemCard(item: items[index]),
)
```

### Platform-Aware Widgets

```dart
import 'dart:io' show Platform;

Widget buildButton() {
  if (Platform.isIOS) {
    return CupertinoButton(
      onPressed: () {},
      child: const Text('iOS Button'),
    );
  }
  return ElevatedButton(
    onPressed: () {},
    child: const Text('Android Button'),
  );
}

// Or use flutter_platform_widgets for automatic adaptation
```

---

## 5. List Patterns

### ListView.builder (Lazy Loading)

```dart
ListView.builder(
  itemCount: items.length,
  itemBuilder: (context, index) {
    return ListTile(
      leading: CircleAvatar(child: Text(items[index].initials)),
      title: Text(items[index].name),
      subtitle: Text(items[index].description),
      trailing: const Icon(Icons.chevron_right),
      onTap: () => context.push('/items/${items[index].id}'),
    );
  },
)
```

### ListView.separated

```dart
ListView.separated(
  itemCount: items.length,
  separatorBuilder: (context, index) => const Divider(height: 1),
  itemBuilder: (context, index) => ItemTile(item: items[index]),
)
```

### Infinite Scroll with Pagination

```dart
class _ProductListState extends State<ProductList> {
  final _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
  }

  void _onScroll() {
    if (_isBottom) {
      context.read<ProductBloc>().add(ProductsFetchedMore());
    }
  }

  bool get _isBottom {
    if (!_scrollController.hasClients) return false;
    final maxScroll = _scrollController.position.maxScrollExtent;
    final currentScroll = _scrollController.offset;
    return currentScroll >= maxScroll - 200; // 200px threshold
  }

  @override
  Widget build(BuildContext context) {
    return BlocBuilder<ProductBloc, ProductState>(
      builder: (context, state) {
        if (state is ProductLoaded) {
          return ListView.builder(
            controller: _scrollController,
            itemCount: state.hasReachedMax
                ? state.products.length
                : state.products.length + 1,
            itemBuilder: (context, index) {
              if (index >= state.products.length) {
                return const Center(child: CircularProgressIndicator());
              }
              return ProductTile(product: state.products[index]);
            },
          );
        }
        return const Center(child: CircularProgressIndicator());
      },
    );
  }

  @override
  void dispose() {
    _scrollController
      ..removeListener(_onScroll)
      ..dispose();
    super.dispose();
  }
}
```

---

## 6. Form Patterns

### Form with Validation

```dart
class LoginForm extends StatefulWidget {
  const LoginForm({super.key});

  @override
  State<LoginForm> createState() => _LoginFormState();
}

class _LoginFormState extends State<LoginForm> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();
  bool _obscurePassword = true;

  @override
  Widget build(BuildContext context) {
    return Form(
      key: _formKey,
      child: Column(
        children: [
          TextFormField(
            controller: _emailController,
            keyboardType: TextInputType.emailAddress,
            textInputAction: TextInputAction.next,
            decoration: const InputDecoration(
              labelText: 'Email',
              prefixIcon: Icon(Icons.email),
              border: OutlineInputBorder(),
            ),
            validator: (value) {
              if (value == null || value.isEmpty) return 'Email is required';
              if (!RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(value)) {
                return 'Enter a valid email';
              }
              return null;
            },
          ),
          const SizedBox(height: 16),
          TextFormField(
            controller: _passwordController,
            obscureText: _obscurePassword,
            textInputAction: TextInputAction.done,
            decoration: InputDecoration(
              labelText: 'Password',
              prefixIcon: const Icon(Icons.lock),
              border: const OutlineInputBorder(),
              suffixIcon: IconButton(
                icon: Icon(
                  _obscurePassword ? Icons.visibility : Icons.visibility_off,
                ),
                onPressed: () =>
                    setState(() => _obscurePassword = !_obscurePassword),
              ),
            ),
            validator: (value) {
              if (value == null || value.isEmpty) return 'Password is required';
              if (value.length < 8) return 'At least 8 characters';
              return null;
            },
          ),
          const SizedBox(height: 24),
          FilledButton(
            onPressed: _submit,
            child: const Text('Login'),
          ),
        ],
      ),
    );
  }

  void _submit() {
    if (_formKey.currentState!.validate()) {
      context.read<AuthBloc>().add(LoginRequested(
        email: _emailController.text.trim(),
        password: _passwordController.text,
      ));
    }
  }

  @override
  void dispose() {
    _emailController.dispose();
    _passwordController.dispose();
    super.dispose();
  }
}
```

---

## 7. Dialog Patterns

### AlertDialog

```dart
showDialog<bool>(
  context: context,
  builder: (context) => AlertDialog(
    title: const Text('Delete Item?'),
    content: const Text('This action cannot be undone.'),
    actions: [
      TextButton(
        onPressed: () => Navigator.of(context).pop(false),
        child: const Text('Cancel'),
      ),
      FilledButton(
        onPressed: () => Navigator.of(context).pop(true),
        child: const Text('Delete'),
      ),
    ],
  ),
).then((confirmed) {
  if (confirmed == true) { /* perform delete */ }
});
```

### Bottom Sheet

```dart
showModalBottomSheet<void>(
  context: context,
  isScrollControlled: true, // For full-height sheets
  useSafeArea: true,
  showDragHandle: true,
  builder: (context) => DraggableScrollableSheet(
    initialChildSize: 0.5,
    minChildSize: 0.25,
    maxChildSize: 0.9,
    expand: false,
    builder: (context, scrollController) {
      return ListView.builder(
        controller: scrollController,
        itemCount: options.length,
        itemBuilder: (context, index) => ListTile(
          title: Text(options[index]),
          onTap: () => Navigator.of(context).pop(),
        ),
      );
    },
  ),
);
```

---

## 8. Animation Widgets

### Implicit Animations (Simple -- Preferred)

```dart
// AnimatedContainer -- animates ALL property changes
AnimatedContainer(
  duration: const Duration(milliseconds: 300),
  curve: Curves.easeInOut,
  width: _expanded ? 200 : 100,
  height: _expanded ? 200 : 100,
  color: _expanded ? Colors.blue : Colors.red,
  child: const FlutterLogo(),
)

// AnimatedOpacity
AnimatedOpacity(
  duration: const Duration(milliseconds: 200),
  opacity: _visible ? 1.0 : 0.0,
  child: const Text('Fade me'),
)

// AnimatedSwitcher -- cross-fade between widgets
AnimatedSwitcher(
  duration: const Duration(milliseconds: 300),
  transitionBuilder: (child, animation) {
    return FadeTransition(opacity: animation, child: child);
  },
  child: Text(
    '$_count',
    key: ValueKey<int>(_count), // Key change triggers animation
  ),
)

// AnimatedAlign, AnimatedPadding, AnimatedPositioned (in Stack)
// AnimatedScale, AnimatedRotation, AnimatedSlide
```

### Explicit Animations (Complex -- Full Control)

```dart
class PulseAnimation extends StatefulWidget {
  const PulseAnimation({super.key});

  @override
  State<PulseAnimation> createState() => _PulseAnimationState();
}

class _PulseAnimationState extends State<PulseAnimation>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _animation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 1),
    )..repeat(reverse: true);

    _animation = Tween<double>(begin: 0.8, end: 1.2).animate(
      CurvedAnimation(parent: _controller, curve: Curves.easeInOut),
    );
  }

  @override
  Widget build(BuildContext context) {
    return ScaleTransition(
      scale: _animation,
      child: const Icon(Icons.favorite, color: Colors.red, size: 48),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }
}
```

### TweenAnimationBuilder (No Controller Needed)

```dart
TweenAnimationBuilder<double>(
  tween: Tween(begin: 0.0, end: _progress),
  duration: const Duration(milliseconds: 500),
  curve: Curves.easeOut,
  builder: (context, value, child) {
    return LinearProgressIndicator(value: value);
  },
)
```

### Hero Animation

```dart
// Source screen
Hero(
  tag: 'product-${product.id}',
  child: Image.network(product.imageUrl, width: 80, height: 80),
)

// Destination screen
Hero(
  tag: 'product-${product.id}',
  child: Image.network(product.imageUrl, width: 300, height: 300),
)
```

### Animation Decision Tree

```
Do you need fine-grained control?
├── NO -> Use Implicit Animation
│   ├── Animating a single property? -> AnimatedFoo (AnimatedOpacity, AnimatedScale, etc.)
│   ├── Animating multiple properties? -> AnimatedContainer
│   └── Switching between widgets? -> AnimatedSwitcher
│
└── YES -> Use Explicit Animation
    ├── Need AnimationController? -> SingleTickerProviderStateMixin
    ├── Repeating/looping? -> controller.repeat()
    ├── Staggered animations? -> Interval + multiple Tweens
    └── One-shot tween? -> TweenAnimationBuilder (no controller)
```

---

## 9. Custom Painting

```dart
class WavePainter extends CustomPainter {
  final double animationValue;
  final Color color;

  WavePainter({required this.animationValue, required this.color});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;

    final path = Path();
    path.moveTo(0, size.height * 0.7);

    for (double i = 0; i <= size.width; i++) {
      path.lineTo(
        i,
        size.height * 0.7 +
            sin((i / size.width * 2 * pi) + (animationValue * 2 * pi)) * 20,
      );
    }

    path.lineTo(size.width, size.height);
    path.lineTo(0, size.height);
    path.close();

    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(WavePainter oldDelegate) =>
      oldDelegate.animationValue != animationValue ||
      oldDelegate.color != color;
}

// Usage
CustomPaint(
  painter: WavePainter(animationValue: _animation.value, color: Colors.blue),
  size: const Size(double.infinity, 200),
)
```

### ClipPath for Custom Shapes

```dart
ClipPath(
  clipper: WaveClipper(),
  child: Container(
    height: 200,
    color: Theme.of(context).colorScheme.primaryContainer,
  ),
)

class WaveClipper extends CustomClipper<Path> {
  @override
  Path getClip(Size size) {
    final path = Path();
    path.lineTo(0, size.height - 40);
    path.quadraticBezierTo(
      size.width / 4, size.height,
      size.width / 2, size.height - 40,
    );
    path.quadraticBezierTo(
      3 * size.width / 4, size.height - 80,
      size.width, size.height - 40,
    );
    path.lineTo(size.width, 0);
    path.close();
    return path;
  }

  @override
  bool shouldReclip(covariant CustomClipper<Path> oldClipper) => false;
}
```

---

## 10. Slivers

### CustomScrollView with Slivers

```dart
CustomScrollView(
  slivers: [
    SliverAppBar.large(
      title: const Text('Products'),
      floating: true,
      pinned: true,
      flexibleSpace: FlexibleSpaceBar(
        background: Image.asset('assets/banner.jpg', fit: BoxFit.cover),
      ),
    ),
    SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Text('Featured', style: Theme.of(context).textTheme.headlineSmall),
      ),
    ),
    SliverList.builder(
      itemCount: featured.length,
      itemBuilder: (context, index) => ProductTile(product: featured[index]),
    ),
    SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Text('All Products', style: Theme.of(context).textTheme.headlineSmall),
      ),
    ),
    SliverGrid.builder(
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 2,
        crossAxisSpacing: 8,
        mainAxisSpacing: 8,
      ),
      itemCount: products.length,
      itemBuilder: (context, index) => ProductCard(product: products[index]),
    ),
    const SliverToBoxAdapter(
      child: SizedBox(height: 80), // Bottom padding
    ),
  ],
)
```

### SliverPersistentHeader (Sticky Headers)

```dart
SliverPersistentHeader(
  pinned: true,
  delegate: _StickyHeaderDelegate(
    minHeight: 50,
    maxHeight: 50,
    child: Container(
      color: Theme.of(context).colorScheme.surface,
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: const Text('Category A'),
    ),
  ),
)

class _StickyHeaderDelegate extends SliverPersistentHeaderDelegate {
  final double minHeight;
  final double maxHeight;
  final Widget child;

  _StickyHeaderDelegate({
    required this.minHeight,
    required this.maxHeight,
    required this.child,
  });

  @override
  double get minExtent => minHeight;
  @override
  double get maxExtent => maxHeight;

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlapsContent) {
    return SizedBox.expand(child: child);
  }

  @override
  bool shouldRebuild(_StickyHeaderDelegate oldDelegate) {
    return maxHeight != oldDelegate.maxHeight ||
        minHeight != oldDelegate.minHeight ||
        child != oldDelegate.child;
  }
}
```

---

## 11. Performance Widgets

### RepaintBoundary

```dart
// Isolate expensive painting from parent repaints
RepaintBoundary(
  child: CustomPaint(painter: ExpensivePainter()),
)
```

### const Constructors

```dart
// ALWAYS mark widgets as const when possible
const SizedBox(height: 16)                      // Good
SizedBox(height: 16)                             // Bad -- recreated each build

const EdgeInsets.all(16)                          // Good
EdgeInsets.all(16)                                // Bad

// Const widget won't rebuild even if parent rebuilds
class MyWidget extends StatelessWidget {
  const MyWidget({super.key});                   // Always add const constructor
}
```

### Keys

```dart
// ValueKey -- for items with a unique business value
ListView.builder(
  itemBuilder: (context, index) => ProductTile(
    key: ValueKey(products[index].id),           // Preserves state across reorders
    product: products[index],
  ),
)

// ObjectKey -- for items identified by object identity
ObjectKey(product)

// UniqueKey -- forces widget recreation (new state every time)
AnimatedSwitcher(
  child: SomeWidget(key: UniqueKey()),           // Always triggers animation
)

// GlobalKey -- access widget state from outside (use sparingly)
final formKey = GlobalKey<FormState>();
formKey.currentState!.validate();
```

---

## 12. Accessibility

```dart
// Semantics widget -- provide screen reader info
Semantics(
  label: 'Add to cart button for ${product.name}',
  button: true,
  child: IconButton(
    icon: const Icon(Icons.add_shopping_cart),
    onPressed: () => addToCart(product),
  ),
)

// ExcludeSemantics -- hide decorative elements
ExcludeSemantics(
  child: Image.asset('assets/decorative_divider.png'),
)

// MergeSemantics -- combine child semantics into one node
MergeSemantics(
  child: ListTile(
    leading: const Icon(Icons.person),
    title: const Text('John Doe'),
    subtitle: const Text('john@example.com'),
  ),
)

// Sufficient touch targets (Material 3: minimum 48x48)
// InkWell, IconButton, etc. already handle this

// Text scaling support
Text(
  'Scalable text',
  style: Theme.of(context).textTheme.bodyLarge,
  // Never set textScaleFactor to a fixed value
)

// Focus and keyboard navigation
Focus(
  autofocus: true,
  onKeyEvent: (node, event) {
    if (event is KeyDownEvent && event.logicalKey == LogicalKeyboardKey.enter) {
      _submit();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  },
  child: const TextField(),
)
```

---

## 13. Common Gotchas

### Unbounded Constraints

```dart
// BAD: ListView inside Column without constraints
Column(
  children: [
    ListView.builder(...), // CRASH: Vertical viewport has unbounded height
  ],
)

// GOOD: Wrap in Expanded or SizedBox
Column(
  children: [
    Expanded(
      child: ListView.builder(...), // Works -- bounded by Expanded
    ),
  ],
)

// GOOD: Use shrinkWrap (only for small lists)
Column(
  children: [
    ListView.builder(
      shrinkWrap: true,            // Calculates its own height
      physics: const NeverScrollableScrollPhysics(),
      ...
    ),
  ],
)
```

### Nested Scrolling

```dart
// BAD: Scrollable inside scrollable without coordination
SingleChildScrollView(
  child: ListView(...), // Gesture conflict
)

// GOOD: Use CustomScrollView with Slivers
CustomScrollView(
  slivers: [
    SliverToBoxAdapter(child: HeaderWidget()),
    SliverList.builder(...),
  ],
)

// GOOD: Use NeverScrollableScrollPhysics on inner scroll
SingleChildScrollView(
  child: ListView(
    shrinkWrap: true,
    physics: const NeverScrollableScrollPhysics(),
    children: [...],
  ),
)
```

### Missing dispose()

```dart
// ALWAYS dispose controllers, streams, subscriptions
@override
void dispose() {
  _controller.dispose();
  _scrollController.dispose();
  _focusNode.dispose();
  _subscription.cancel();
  _streamController.close();
  super.dispose();
}
```

### BuildContext Usage After Async Gaps

```dart
// BAD: Using context after await (might be unmounted)
Future<void> _save() async {
  await repository.save(data);
  ScaffoldMessenger.of(context).showSnackBar(...); // Might crash
}

// GOOD: Check mounted
Future<void> _save() async {
  await repository.save(data);
  if (!mounted) return;
  ScaffoldMessenger.of(context).showSnackBar(...);
}
```

### setState() in StatelessWidget

```dart
// You can't call setState in StatelessWidget. Use:
// 1. Convert to StatefulWidget
// 2. Use a state management solution (BLoC, Riverpod, etc.)
// 3. Use ValueNotifier + ValueListenableBuilder for simple cases:

class MyWidget extends StatelessWidget {
  final ValueNotifier<int> _counter = ValueNotifier(0);

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<int>(
      valueListenable: _counter,
      builder: (context, value, child) {
        return TextButton(
          onPressed: () => _counter.value++,
          child: Text('Count: $value'),
        );
      },
    );
  }
}
```

### Image Loading Best Practices

```dart
// Always provide placeholder and error handling
Image.network(
  url,
  loadingBuilder: (context, child, progress) {
    if (progress == null) return child;
    return Center(
      child: CircularProgressIndicator(
        value: progress.expectedTotalBytes != null
            ? progress.cumulativeBytesLoaded / progress.expectedTotalBytes!
            : null,
      ),
    );
  },
  errorBuilder: (context, error, stackTrace) {
    return const Icon(Icons.broken_image, size: 48);
  },
)

// Use cached_network_image for production
CachedNetworkImage(
  imageUrl: url,
  placeholder: (context, url) => const CircularProgressIndicator(),
  errorWidget: (context, url, error) => const Icon(Icons.error),
)
```

### MediaQuery Best Practices

```dart
// GOOD: Use specific methods (avoids unnecessary rebuilds)
final width = MediaQuery.sizeOf(context).width;
final padding = MediaQuery.paddingOf(context);
final viewInsets = MediaQuery.viewInsetsOf(context);

// BAD: Full MediaQuery (rebuilds on ANY change)
final data = MediaQuery.of(context);
final width = data.size.width;
```
