/// Shared map base style definition for Flutter.
///
/// - Persist to storage using [name] (e.g. "normal")
/// - Inject to WebView using [url]
/// - Display labels are hardcoded in map_settings_page.dart like other pages
enum MapBaseStyle {
  normal,
  satellite,
  hybrid;

  static MapBaseStyle fromName(String? name) {
    return MapBaseStyle.values.firstWhere(
      (e) => e.name == name,
      orElse: () => MapBaseStyle.normal,
    );
  }

  String get url => switch (this) {
        MapBaseStyle.normal => "https://tiles.openfreemap.org/styles/liberty",
        MapBaseStyle.satellite => "mapbox://styles/mapbox/satellite-v9",
        MapBaseStyle.hybrid => "mapbox://styles/mapbox/satellite-streets-v12",
      };
}
