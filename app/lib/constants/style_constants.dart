import 'dart:ui';

class StyleConstants {
  StyleConstants._();

  // navBar
  static const double navBarHeight = 64;
  static const double navBarBottomPadding = 32;
  static const double navBarHorizontalPadding = 24;
  static const double navBarSafeArea = navBarHeight + navBarBottomPadding;
  static const Color defaultColor = Color(0xFFB4EC51);

  /// 浮窗/卡片圆角，与底部栏、选项卡等统一
  static const double overlayFloatingRadius = 16.0;
}
