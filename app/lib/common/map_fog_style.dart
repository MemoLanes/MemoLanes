/// Visual parameters for the unexplored-area overlay drawn above the base map.
///
/// Fog styling is intentionally independent from [MapStyle]. This allows the
/// app to switch light and dark fog appearances without coupling them to a
/// particular tile provider.
class MapFogStyle {
  const MapFogStyle({
    required this.id,
    required this.colorHex,
    required this.opacity,
  });

  /// Stable identifier persisted in app preferences.
  final String id;

  /// A CSS-compatible six-digit RGB color.
  final String colorHex;

  /// Overlay opacity in the inclusive range 0–1.
  final double opacity;

  static const dark = MapFogStyle(
    id: 'dark',
    colorHex: '#001228',
    opacity: 0.50,
  );

  static const light = MapFogStyle(
    id: 'light',
    colorHex: '#C0D7E2',
    opacity: 0.60,
  );

  static const all = [dark, light];

  static MapFogStyle findById(String? id) {
    return all.firstWhere(
      (style) => style.id == id,
      orElse: () => dark,
    );
  }
}
