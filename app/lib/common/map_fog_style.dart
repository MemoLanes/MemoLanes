/// Visual parameters for the unexplored-area overlay drawn above the base map.
///
/// Fog styling is intentionally independent from [MapStyle]. This allows the
/// app to switch light and dark fog appearances without coupling them to a
/// particular tile provider.
enum MapFogStyle {
  dark('dark'),
  light('light');

  const MapFogStyle(this.id);

  /// Stable identifier persisted in app preferences and passed to the renderer.
  final String id;

  static const all = [dark, light];

  static MapFogStyle findById(String? id) {
    return all.firstWhere((style) => style.id == id, orElse: () => dark);
  }
}
