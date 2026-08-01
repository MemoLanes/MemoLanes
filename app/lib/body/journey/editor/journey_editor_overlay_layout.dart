import 'package:flutter/widgets.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Geometry shared by the floating controls in the journey track editor.
class JourneyEditorOverlayLayout {
  JourneyEditorOverlayLayout._();

  static const double modeBarExtent = 64.0;
  static const double minimumHorizontalInset = 16.0;

  static double modeBarBottomInset(BuildContext context) =>
      StyleConstants.navBarBottomInset(context);

  /// Space at the bottom of the map that the persistent mode bar covers.
  static double mapBottomOverlayExtent(BuildContext context) =>
      modeBarBottomInset(context) + modeBarExtent;
}
