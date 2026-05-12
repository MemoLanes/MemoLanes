/// Registered by [MyHomePage] so deep links (e.g. Live Activity) can select the map tab.
class HomeTabBridge {
  HomeTabBridge._();

  static void Function(int index)? selectTab;

  static void openMapTab() {
    selectTab?.call(0);
  }
}
