# Advanced Animation Patterns Reference

> Physics-based animations, CustomPainter, flutter_animate, Rive state machines,
> staggered lists, TweenAnimationBuilder, and GoRouter page transitions.

---

## 1. Physics-Based Animations

### SpringSimulation

```dart
class _SpringCardState extends State<SpringCard>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this);
    final spring = SpringDescription(mass: 1, stiffness: 200, damping: 10);
    final simulation = SpringSimulation(spring, 0, 1, 0);
    _controller.animateWith(simulation);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, child) => Transform.scale(
        scale: _controller.value,
        child: child,
      ),
      child: const Card(child: Padding(
        padding: EdgeInsets.all(24),
        child: Text('Spring!'),
      )),
    );
  }
}
```

### FrictionSimulation

```dart
void _fling(double velocity) {
  final simulation = FrictionSimulation(0.135, _position, velocity);
  _controller.animateWith(simulation);
}
```

### GravitySimulation

```dart
final simulation = GravitySimulation(
  9.81 * 100, // acceleration (pixels/s^2)
  0, 500, 0,  // start, end, velocity
);
_controller.animateWith(simulation);
```

---

## 2. CustomPainter with Animation

```dart
class WavePainter extends CustomPainter {
  final Animation<double> animation;
  WavePainter({required this.animation}) : super(repaint: animation);

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.blue.withOpacity(0.6)
      ..style = PaintingStyle.fill;

    final path = Path()..moveTo(0, size.height);
    for (double x = 0; x <= size.width; x++) {
      final y = size.height * 0.5 +
          sin((x / size.width * 2 * pi) + (animation.value * 2 * pi)) * 30;
      path.lineTo(x, y);
    }
    path.lineTo(size.width, size.height);
    path.close();
    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(WavePainter oldDelegate) => true;
}
```

---

## 3. flutter_animate Declarative Patterns

```yaml
dependencies:
  flutter_animate: ^4.5.0
```

```dart
import 'package:flutter_animate/flutter_animate.dart';

// Fade + slide entrance
Text('Hello')
    .animate()
    .fadeIn(duration: 600.ms)
    .slideY(begin: 0.3, end: 0, curve: Curves.easeOutCubic)

// Staggered list items
ListView.builder(
  itemCount: items.length,
  itemBuilder: (context, index) {
    return ListTile(title: Text(items[index]))
        .animate()
        .fadeIn(delay: (index * 100).ms, duration: 400.ms)
        .slideX(begin: 0.2, end: 0);
  },
)

// Shimmer loading
Container(width: 200, height: 20, color: Colors.grey[300])
    .animate(onPlay: (c) => c.repeat())
    .shimmer(duration: 1200.ms, color: Colors.grey[100]!)

// Scale on tap
Icon(Icons.favorite)
    .animate(controller: _controller, autoPlay: false)
    .scale(begin: 1, end: 1.3, duration: 200.ms)
    .then()
    .scale(begin: 1.3, end: 1, duration: 200.ms)

// Complex entrance
Card(child: content)
    .animate()
    .fadeIn(duration: 400.ms)
    .slideY(begin: 0.1, end: 0)
    .scaleXY(begin: 0.95, end: 1)
    .blurXY(begin: 4, end: 0)
```

---

## 4. Rive Interactive State Machines

```dart
import 'package:rive/rive.dart';

class _RiveButtonState extends State<RiveButton> {
  StateMachineController? _controller;
  SMITrigger? _pressed;
  SMIBool? _isHovered;

  void _onRiveInit(Artboard artboard) {
    _controller = StateMachineController.fromArtboard(artboard, 'ButtonState');
    if (_controller != null) {
      artboard.addController(_controller!);
      _pressed = _controller!.findInput<bool>('pressed') as SMITrigger?;
      _isHovered = _controller!.findInput<bool>('hovered') as SMIBool?;
    }
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => _isHovered?.value = true,
      onExit: (_) => _isHovered?.value = false,
      child: GestureDetector(
        onTap: () => _pressed?.fire(),
        child: SizedBox(
          width: 200, height: 60,
          child: RiveAnimation.asset(
            'assets/animations/button.riv',
            onInit: _onRiveInit,
            fit: BoxFit.contain,
          ),
        ),
      ),
    );
  }
}
```

---

## 5. TweenAnimationBuilder

```dart
// Animated counter
TweenAnimationBuilder<double>(
  tween: Tween<double>(begin: 0, end: totalAmount),
  duration: const Duration(milliseconds: 800),
  curve: Curves.easeOutCubic,
  builder: (context, value, _) {
    return Text('\$${value.toStringAsFixed(2)}');
  },
)

// Animated color
TweenAnimationBuilder<Color?>(
  tween: ColorTween(begin: Colors.blue, end: isActive ? Colors.green : Colors.red),
  duration: const Duration(milliseconds: 300),
  builder: (context, color, child) {
    return Container(color: color, padding: const EdgeInsets.all(16), child: child);
  },
  child: const Text('Status'),
)
```

---

## 6. GoRouter Page Transitions

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

// Platform-adaptive
transitionsBuilder: (context, animation, secondaryAnimation, child) {
  final isIOS = Theme.of(context).platform == TargetPlatform.iOS;
  if (isIOS) {
    return CupertinoPageTransition(
      primaryRouteAnimation: animation,
      secondaryRouteAnimation: secondaryAnimation,
      linearTransition: false,
      child: child,
    );
  }
  return FadeTransition(opacity: animation, child: child);
}
```

---

## 7. Animation Decision Tree

| Need | Solution |
|---|---|
| Simple property change | Implicit animations (AnimatedContainer, AnimatedOpacity) |
| Custom property interpolation | TweenAnimationBuilder |
| Complex multi-step sequence | AnimationController + Interval |
| Physics (spring, fling, gravity) | AnimationController.animateWith(Simulation) |
| Declarative chaining | flutter_animate package |
| Interactive vector animations | Rive state machines |
| List entrance animations | Staggered with Interval or flutter_animate |
| Route transitions | CustomTransitionPage in GoRouter |
| Shared element transitions | Hero widget |
| Canvas/custom drawing | CustomPainter with repaint: animation |

### Performance Rules

- Use RepaintBoundary around expensive animated widgets
- Prefer Transform over Container for GPU-accelerated transforms
- Avoid Opacity widget -- use FadeTransition instead
- Keep animations under 16ms per frame (8ms for 120Hz)
- Profile in --profile mode, never debug mode
