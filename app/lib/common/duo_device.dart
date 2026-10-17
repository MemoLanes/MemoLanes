import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    as native
    show DuoBarEdge, DuoBridge, DuoHardware, DuoHardwareScope;
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

/// A physical window edge, resolved by UIKit rather than screen aspect ratio.
enum DuoBarEdge { unknown, horizontal, left, right }

/// Hardware stays above the navigator so pushed editors and dialogs see changes.
/// No display-feature bridge: framework dialogs keep their existing behaviour.
class DuoDeviceProvider extends StatelessWidget {
  const DuoDeviceProvider({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => native.DuoHardwareScope(
    bridge: native.DuoBridge.none,
    hardware: defaultTargetPlatform == TargetPlatform.iOS
        ? null
        : native.DuoHardware.none,
    child: _DuoDeviceData(child: child),
  );
}

class DuoBarScope extends InheritedWidget {
  const DuoBarScope({super.key, required this.edge, required super.child});

  final DuoBarEdge edge;

  static DuoBarEdge of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DuoBarScope>()?.edge ??
      DuoBarEdge.unknown;

  @override
  bool updateShouldNotify(DuoBarScope oldWidget) => edge != oldWidget.edge;
}

class _DuoDeviceData extends StatelessWidget {
  const _DuoDeviceData({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final hardware = native.DuoHardware.of(context);
    // API availability alone also includes ordinary iPhones and iPads.
    final isDuo =
        defaultTargetPlatform == TargetPlatform.iOS && hardware.hasHinge;
    final edge = isDuo
        ? switch (hardware.barEdge) {
            native.DuoBarEdge.left => DuoBarEdge.left,
            native.DuoBarEdge.right => DuoBarEdge.right,
            native.DuoBarEdge.none => DuoBarEdge.horizontal,
          }
        : DuoBarEdge.unknown;
    final ambient = MediaQuery.of(context);
    // Once the engine supplies iOS features, RawDialogRoute would otherwise
    // split the navigator before our native-region placement runs a second time.
    return MediaQuery(
      data: isDuo ? ambient.copyWith(displayFeatures: const []) : ambient,
      child: DuoBarScope(edge: edge, child: child),
    );
  }
}
