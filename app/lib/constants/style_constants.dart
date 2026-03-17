import 'dart:ui';

class StyleConstants {
  StyleConstants._();

  // navBar
  static const double navBarHeight = 64;
  static const double navBarBottomPadding = 32;
  static const double navBarHorizontalPadding = 24;
  static const double navBarSafeArea = navBarHeight + navBarBottomPadding;

  // colors
  static const Color defaultColor = Color(0xFFB4EC51);
  static const Color loadingMaskColor = Color.fromRGBO(0, 0, 0, 0.35);
  static const double overlayFloatingRadius = 16.0;
}
