import 'dart:ui';

import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';

class StyleConstants {
  StyleConstants._();

  // navBar
  // Fixed bottom inset for the nav bar. It includes the map copyright button
  // below the nav bar plus the small gap between them.
  static const double navBarBottomGap = MapCopyrightButton.bottomGap +
      MapCopyrightButton.buttonSize +
      MapCopyrightButton.navBarSpacing;

  // Vertical space occupied by the nav bar and its fixed bottom inset.
  // Scrollable pages use this to keep content clear of the floating nav bar.
  static const double navBarSafeArea = BottomNavBar.height + navBarBottomGap;

  // Gap between the nav bar and primary map controls such as recording buttons
  // and the time-machine ruler.
  static const double mapPrimaryControlNavBarSpacing = 20;

  // Bottom inset shared by primary map controls so they align across map modes.
  static const double mapPrimaryControlBottomInset =
      navBarSafeArea + mapPrimaryControlNavBarSpacing;

  // colors
  static const Color defaultColor = Color(0xFFB4EC51);
  static const Color loadingMaskColor = Color.fromRGBO(0, 0, 0, 0.35);
  static const double overlayFloatingRadius = 16.0;
}
