import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class StyleConstants {
  StyleConstants._();

  // TODO: `BackdropFilter` is very expensive and uses a lot of memory.
  // We disable it for now. We should try to optimize it, or use it only when
  // necessary or disable it when entering background.
  static const bool enableBackdropFilter = false;

  // Switch geometry; colors live in AppColors.
  static const double switchActiveThumbSize = 18;
  static const double switchInactiveThumbSize = 16;
  static const double switchTrackOutlineWidth = 1;

  // navBar
  // Kept in the layout constants layer so shared safe-area calculations do
  // not need to depend on the BottomNavBar widget implementation.
  static const double navBarHeight = 58;

  // Visual bottom inset for the floating nav bar on gesture/home-indicator
  // devices. This intentionally differs from the raw safe-area value so iOS
  // and Android look closer while still clearing bottom rounded corners.
  static const double navBarGestureBottomInset = 32;

  // Gap above a non-gesture system navigation area, such as Android 3-button
  // navigation.
  static const double navBarSystemAreaGap = 5;

  // Fallback inset for screens without a reported bottom system area.
  static const double navBarMinimumBottomInset = 32;

  // Vertical space occupied by the nav bar and its fixed bottom inset.
  // Scrollable pages use this to keep content clear of the floating nav bar.
  static const double navBarSafeArea = navBarHeight + navBarMinimumBottomInset;

  // Gap between the nav bar and primary map controls such as recording buttons
  // and the time-machine ruler.
  static const double mapPrimaryControlNavBarSpacing = 14;

  // Bottom inset shared by primary map controls so they align across map modes.
  static const double mapPrimaryControlBottomInset =
      navBarSafeArea + mapPrimaryControlNavBarSpacing;

  static double navBarBottomInset(BuildContext context) {
    final bottomGestureInset = MediaQuery.systemGestureInsetsOf(context).bottom;
    final bottomSafeArea = MediaQuery.viewPaddingOf(context).bottom;

    return switch ((
      bottomGestureInset,
      bottomSafeArea,
      defaultTargetPlatform,
    )) {
      (> 0, _, _) => bottomGestureInset + navBarGestureBottomInset,
      (_, > 0, TargetPlatform.iOS) => navBarGestureBottomInset,
      (_, > 0, _) => bottomSafeArea + navBarSystemAreaGap,
      _ => navBarMinimumBottomInset,
    };
  }

  static double navBarSafeAreaForContext(BuildContext context) =>
      navBarHeight + navBarBottomInset(context);

  static double mapPrimaryControlBottomInsetForContext(BuildContext context) =>
      navBarSafeAreaForContext(context) + mapPrimaryControlNavBarSpacing;

  // Overlays
  static const double overlayFloatingRadius = 16.0;

  // Shared elevation for glass buttons and cards displayed over the map.
  static const double mapOverlayShadowBlurRadius = 26;
  static const double mapOverlayShadowSpreadRadius = -3;
  static const Offset mapOverlayShadowOffset = Offset(0, 8);

  // Calm, readable glass used by the time-machine ruler and other secondary
  // map controls that should remain visible without looking like solid cards.
  static const double timelineGlassBlurSigma = 24;
}
