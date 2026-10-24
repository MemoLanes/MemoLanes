import 'dart:async';

import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    as native
    show DuoBarEdge, DuoBridge, DuoHardware, DuoHardwareScope;
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

/// A physical window edge, resolved by UIKit rather than screen aspect ratio.
enum DuoBarEdge { unknown, horizontal, left, right }

/// Hardware stays above the navigator so pushed editors and dialogs see changes.
/// No display-feature bridge: framework dialogs keep their existing behaviour.
class DuoDeviceProvider extends StatefulWidget {
  const DuoDeviceProvider({super.key, required this.child});

  final Widget child;

  @override
  State<DuoDeviceProvider> createState() => _DuoDeviceProviderState();
}

class _DuoDeviceProviderState extends State<DuoDeviceProvider> {
  StreamSubscription<native.DuoHardware>? _subscription;
  native.DuoHardware? _hardware;

  @override
  void initState() {
    super.initState();
    if (defaultTargetPlatform != TargetPlatform.iOS) {
      _hardware = native.DuoHardware.none;
      return;
    }
    _subscription = native.DuoHardware.stream.listen(
      _received,
      onError: (Object error) => _finishInitialization(),
      onDone: _finishInitialization,
    );
  }

  void _received(native.DuoHardware hardware) {
    if (mounted && hardware != _hardware) {
      setState(() => _hardware = hardware);
    }
  }

  void _finishInitialization() {
    if (_hardware == null) _received(native.DuoHardware.none);
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final hardware = _hardware ?? native.DuoHardware.none;
    // API availability alone also includes ordinary iPhones and iPads.
    final isDuo =
        defaultTargetPlatform == TargetPlatform.iOS && hardware.hasHinge;
    final edge = switch (hardware.barEdge) {
      native.DuoBarEdge.left => DuoBarEdge.left,
      native.DuoBarEdge.right => DuoBarEdge.right,
      native.DuoBarEdge.none =>
        isDuo ? DuoBarEdge.horizontal : DuoBarEdge.unknown,
    };
    final ambient = MediaQuery.of(context);
    // Supplying hardware disables the scope's own subscription. Keep angle
    // filtering and inherited data in the plugin without listening twice.
    return native.DuoHardwareScope(
      bridge: native.DuoBridge.none,
      hardware: hardware,
      child: MediaQuery(
        // Native placement owns folds even if the bar edge precedes the hinge.
        data: edge == DuoBarEdge.unknown
            ? ambient
            : ambient.copyWith(displayFeatures: const []),
        child: DuoBarScope(
          edge: edge,
          isReady: _hardware != null,
          child: widget.child,
        ),
      ),
    );
  }
}

class DuoBarScope extends InheritedWidget {
  const DuoBarScope({
    super.key,
    required this.edge,
    this.isReady = true,
    required super.child,
  });

  final DuoBarEdge edge;

  /// A first plugin response has arrived; it may still lack native details.
  final bool isReady;

  static DuoBarEdge of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DuoBarScope>()?.edge ??
      DuoBarEdge.unknown;

  static bool isReadyOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DuoBarScope>()?.isReady ??
      true;

  @override
  bool updateShouldNotify(DuoBarScope oldWidget) =>
      edge != oldWidget.edge || isReady != oldWidget.isReady;
}
