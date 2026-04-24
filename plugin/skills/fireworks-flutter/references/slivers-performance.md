# Slivers and Performance Optimization Reference

> CustomScrollView composition, SliverFixedExtentList, SliverPersistentHeader,
> 120Hz frame budget, Isolate.run, RepaintBoundary, image optimization, Impeller profiling.

---

## 1. CustomScrollView Composition

```dart
CustomScrollView(
  slivers: [
    // Collapsible app bar
    SliverAppBar(
      expandedHeight: 200,
      pinned: true,
      flexibleSpace: FlexibleSpaceBar(
        title: const Text('Products'),
        background: Image.network(heroUrl, fit: BoxFit.cover),
      ),
    ),

    // Pinned section header
    SliverPersistentHeader(
      pinned: true,
      delegate: _SectionHeaderDelegate(
        minHeight: 48,
        maxHeight: 48,
        child: Container(
          color: Theme.of(context).colorScheme.surface,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          alignment: Alignment.centerLeft,
          child: const Text('Featured', style: TextStyle(fontWeight: FontWeight.bold)),
        ),
      ),
    ),

    // Fixed extent list (best performance for uniform height items)
    SliverFixedExtentList(
      itemExtent: 72,
      delegate: SliverChildBuilderDelegate(
        (context, index) => ProductTile(product: products[index]),
        childCount: products.length,
      ),
    ),

    // Grid section
    SliverGrid(
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 2,
        mainAxisSpacing: 8,
        crossAxisSpacing: 8,
        childAspectRatio: 0.75,
      ),
      delegate: SliverChildBuilderDelegate(
        (context, index) => ProductCard(product: gridProducts[index]),
        childCount: gridProducts.length,
      ),
    ),

    // Padding at bottom
    const SliverPadding(
      padding: EdgeInsets.only(bottom: 80),
      sliver: SliverToBoxAdapter(child: SizedBox.shrink()),
    ),
  ],
)
```

### SliverPersistentHeader Delegate

```dart
class _SectionHeaderDelegate extends SliverPersistentHeaderDelegate {
  final double minHeight;
  final double maxHeight;
  final Widget child;

  _SectionHeaderDelegate({
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
  bool shouldRebuild(_SectionHeaderDelegate oldDelegate) {
    return maxHeight != oldDelegate.maxHeight ||
        minHeight != oldDelegate.minHeight ||
        child != oldDelegate.child;
  }
}
```

---

## 2. SliverFixedExtentList vs SliverList

| Widget | When to Use | Why |
|---|---|---|
| `SliverFixedExtentList` | All items same height | Skips layout measurement -- O(1) scroll offset |
| `SliverPrototypeExtentList` | Items similar height | Measures one prototype, assumes rest match |
| `SliverList` | Variable height items | Measures each item -- slower for large lists |
| `SliverGrid` | Grid layouts | Cross-axis arrangement |

```dart
// Best: fixed extent (uniform height)
SliverFixedExtentList(
  itemExtent: 72,
  delegate: SliverChildBuilderDelegate(
    (context, index) => ListTile(title: Text(items[index])),
    childCount: items.length,
  ),
)

// Good: prototype extent (similar height)
SliverPrototypeExtentList(
  prototypeItem: const ListTile(title: Text('Prototype')),
  delegate: SliverChildBuilderDelegate(
    (context, index) => ListTile(title: Text(items[index])),
    childCount: items.length,
  ),
)
```

---

## 3. Frame Budget (60Hz vs 120Hz)

| Refresh Rate | Frame Budget | Target |
|---|---|---|
| 60Hz | 16.67ms | Standard devices |
| 90Hz | 11.11ms | Mid-range modern phones |
| 120Hz | 8.33ms | Flagship phones, iPad Pro |

### Profiling Frame Times

```bash
# Profile mode (NOT debug!)
flutter run --profile

# Trace timeline
flutter run --profile --trace-startup
```

### Rules for Hitting 120Hz

1. Keep build() under 4ms
2. Move computation off UI thread (Isolate.run)
3. Use `const` widgets everywhere possible
4. Avoid deep widget trees (flatten with `CustomMultiChildLayout`)
5. Use `RepaintBoundary` around independently animated sections

---

## 4. Isolate.run() for Heavy Computation

```dart
// Parse large JSON off the main thread
Future<List<Product>> parseProducts(String jsonString) async {
  return await Isolate.run(() {
    final List<dynamic> data = jsonDecode(jsonString);
    return data.map((e) => Product.fromJson(e as Map<String, dynamic>)).toList();
  });
}

// Image processing
Future<Uint8List> processImage(Uint8List imageBytes) async {
  return await Isolate.run(() {
    // Heavy image manipulation here
    return _applyFilter(imageBytes);
  });
}

// Complex sorting/filtering
Future<List<Product>> sortAndFilter(
  List<Product> products,
  String query,
  SortOrder order,
) async {
  return await Isolate.run(() {
    var filtered = products.where((p) =>
        p.name.toLowerCase().contains(query.toLowerCase())).toList();
    filtered.sort((a, b) => order == SortOrder.asc
        ? a.price.compareTo(b.price)
        : b.price.compareTo(a.price));
    return filtered;
  });
}
```

### When to Use Isolate.run

| Task | Use Isolate? |
|---|---|
| JSON parsing (< 100 items) | No |
| JSON parsing (1000+ items) | Yes |
| Image encoding/decoding | Yes |
| Sorting large lists (500+) | Yes |
| String search in small data | No |
| Crypto operations | Yes |
| Simple calculations | No |

---

## 5. RepaintBoundary

```dart
// Wrap independently animated sections
RepaintBoundary(
  child: AnimatedWidget(/* ... */),
)

// Wrap expensive static content
RepaintBoundary(
  child: ComplexChart(data: chartData),
)

// Verify with debug flag
debugRepaintRainbowEnabled = true;
// Areas that flash = being repainted
// RepaintBoundary isolates repaints to its subtree
```

### Where to Place RepaintBoundary

- Around animated widgets (progress bars, loaders)
- Around complex CustomPaint widgets
- Around list items in a scrolling list
- Between sections that update independently
- NOT around everything (adds compositing overhead)

---

## 6. Image Optimization

```dart
// Resize on decode (saves memory)
Image.network(
  imageUrl,
  cacheWidth: 400,   // Decode at this width
  cacheHeight: 400,  // Decode at this height
  // Original 4000x4000 image uses ~64MB in memory
  // Resized 400x400 uses ~640KB
)

// With cached_network_image
CachedNetworkImage(
  imageUrl: imageUrl,
  memCacheWidth: 400,
  memCacheHeight: 400,
  placeholder: (context, url) => const ShimmerPlaceholder(),
  errorWidget: (context, url, error) => const Icon(Icons.error),
)

// Precache images
@override
void didChangeDependencies() {
  super.didChangeDependencies();
  precacheImage(NetworkImage(heroImageUrl), context);
}
```

---

## 7. Impeller-Specific Profiling

Flutter 3.38+ uses Impeller by default on iOS and Android.

```bash
# Check if Impeller is enabled
flutter run --verbose | grep -i impeller

# Force Impeller off (for comparison)
flutter run --no-enable-impeller

# Profile with Impeller
flutter run --profile
# Open DevTools > Performance
# Look for: Raster thread (Impeller) instead of GPU thread
```

### Impeller vs Skia Differences

| Aspect | Impeller | Skia |
|---|---|---|
| Shader compilation | Pre-compiled, no jank | JIT, first-frame jank |
| Text rendering | Glyph atlas | Glyph cache |
| Blur effects | Hardware-optimized | Software fallback possible |
| Path rendering | Tessellation-based | Scanline-based |

### Impeller Optimization Tips

- Avoid `saveLayer` (Opacity widget, ColorFiltered) -- expensive with Impeller
- Use `FadeTransition` instead of `Opacity`
- Blur effects are cheaper on Impeller than Skia
- Complex clip paths are more expensive -- simplify where possible
- Monitor raster thread time in DevTools, not GPU thread

---

## 8. Performance Checklist

- [ ] Lists use `ListView.builder` (not `ListView(children: [])`)
- [ ] Fixed-height lists use `SliverFixedExtentList` or `itemExtent`
- [ ] Images use `cacheWidth`/`cacheHeight`
- [ ] Heavy computation uses `Isolate.run()`
- [ ] Animated widgets wrapped in `RepaintBoundary`
- [ ] `const` constructors used on all eligible widgets
- [ ] No `Opacity` widget -- use `FadeTransition`
- [ ] Profiled in `--profile` mode (not debug)
- [ ] Frame times under budget (16ms for 60Hz, 8ms for 120Hz)
- [ ] No jank on first frame (Impeller eliminates shader compilation jank)
