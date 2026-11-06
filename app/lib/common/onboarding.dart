import 'dart:async';

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

enum OnboardingAnchor {
  recording,
  mapControls,
  timeRange,
  journeys,
  journeyRecord,
  achievements,
  settings,
}

class OnboardingStep {
  const OnboardingStep(this.anchor, this.tab, this.translationKey);

  final OnboardingAnchor anchor;
  final int tab;
  final String translationKey;
}

/// Only controls the tour. Highlighted controls never execute their actions.
class OnboardingController extends ChangeNotifier {
  OnboardingController({required this.selectTab});

  static const steps = [
    OnboardingStep(OnboardingAnchor.recording, 0, 'recording'),
    OnboardingStep(OnboardingAnchor.mapControls, 0, 'map_controls'),
    OnboardingStep(OnboardingAnchor.timeRange, 1, 'time_range'),
    OnboardingStep(OnboardingAnchor.journeys, 2, 'journeys'),
    OnboardingStep(OnboardingAnchor.journeyRecord, 2, 'journey_record'),
    OnboardingStep(OnboardingAnchor.achievements, 3, 'achievements'),
    OnboardingStep(OnboardingAnchor.settings, 4, 'settings'),
  ];

  final ValueChanged<int> selectTab;
  final _targets = <OnboardingAnchor, ({Object owner, Rect bounds})>{};
  Completer<bool>? _completion;
  int _index = 0;
  bool _disposed = false;
  bool _notificationScheduled = false;

  bool get isActive => _completion != null;
  int get index => _index;
  OnboardingStep get step => steps[_index];
  Rect? get targetBounds => _targets[step.anchor]?.bounds;

  Future<bool> start() {
    if (_completion != null) return _completion!.future;
    final completion = _completion = Completer<bool>();
    _index = 0;
    selectTab(step.tab);
    notifyListeners();
    return completion.future;
  }

  void next() {
    if (!isActive) return;
    if (_index == steps.length - 1) {
      finish();
    } else {
      _goTo(_index + 1);
    }
  }

  void previous() {
    if (isActive && _index > 0) _goTo(_index - 1);
  }

  void _goTo(int index) {
    _index = index;
    selectTab(step.tab);
    notifyListeners();
  }

  /// Both Skip and Done count as an explicit dismissal.
  void finish() {
    final completion = _completion;
    if (completion == null) return;
    _completion = null;
    completion.complete(true);
    notifyListeners();
  }

  void _report(OnboardingAnchor anchor, Object owner, Rect bounds) {
    if (_disposed) return;
    final previous = _targets[anchor];
    _targets[anchor] = (owner: owner, bounds: bounds);
    if (previous?.bounds != bounds && isActive && step.anchor == anchor) {
      _scheduleNotification();
    }
  }

  void _remove(OnboardingAnchor anchor, Object owner) {
    if (_disposed || _targets[anchor]?.owner != owner) return;
    _targets.remove(anchor);
    if (isActive && step.anchor == anchor) _scheduleNotification();
  }

  // Targets report during paint/detach; never rebuild the shell mid-frame.
  void _scheduleNotification() {
    if (_notificationScheduled) return;
    _notificationScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _notificationScheduled = false;
      if (!_disposed && isActive) notifyListeners();
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  @override
  void dispose() {
    _disposed = true;
    _completion?.complete(false);
    _completion = null;
    _targets.clear();
    super.dispose();
  }
}

class OnboardingScope extends InheritedWidget {
  const OnboardingScope({
    super.key,
    required this.controller,
    required super.child,
  });

  final OnboardingController controller;

  static OnboardingController? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<OnboardingScope>()?.controller;

  @override
  bool updateShouldNotify(OnboardingScope oldWidget) =>
      controller != oldWidget.controller;
}

/// Measures the actual painted widget, including scrolling and rotation.
/// It adds no padding, gesture handlers, or layout constraints.
class OnboardingTarget extends SingleChildRenderObjectWidget {
  const OnboardingTarget({
    super.key,
    required this.anchor,
    required super.child,
  });

  final OnboardingAnchor anchor;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _OnboardingTargetBox(OnboardingScope.maybeOf(context), anchor);

  @override
  void updateRenderObject(BuildContext context, RenderObject renderObject) =>
      (renderObject as _OnboardingTargetBox).update(
        OnboardingScope.maybeOf(context),
        anchor,
      );
}

class _OnboardingTargetBox extends RenderProxyBox {
  _OnboardingTargetBox(this._controller, this._anchor);

  OnboardingController? _controller;
  OnboardingAnchor _anchor;

  void update(OnboardingController? controller, OnboardingAnchor anchor) {
    if (_controller == controller && _anchor == anchor) return;
    _controller?._remove(_anchor, this);
    _controller = controller;
    _anchor = anchor;
    markNeedsPaint();
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    super.paint(context, offset);
    final bounds = MatrixUtils.transformRect(
      getTransformTo(null),
      Offset.zero & size,
    );
    if (bounds.isEmpty || !bounds.isFinite) {
      _controller?._remove(_anchor, this);
    } else {
      _controller?._report(_anchor, this, bounds);
    }
  }

  @override
  void detach() {
    _controller?._remove(_anchor, this);
    super.detach();
  }
}
