import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

enum OnboardingStep {
  recording(0, 'recording'),
  mapControls(0, 'map_controls'),
  timeRange(1, 'time_range'),
  journeys(2, 'journeys'),
  journeyRecord(2, 'journey_record'),
  achievements(3, 'achievements'),
  settings(4, 'settings');

  const OnboardingStep(this.tab, this.translationKey);

  final int tab;
  final String translationKey;
}

class OnboardingController extends ChangeNotifier {
  OnboardingController({required this.finish});

  final VoidCallback finish;
  int _index = 0;
  Rect? _targetBounds;
  bool _boundsNotificationScheduled = false;
  bool _disposed = false;

  int get index => _index;
  OnboardingStep get step => OnboardingStep.values[_index];
  Rect? get targetBounds => _targetBounds;

  void next() {
    if (_index == OnboardingStep.values.length - 1) {
      finish();
    } else {
      _goTo(_index + 1);
    }
  }

  void previous() {
    if (_index > 0) _goTo(_index - 1);
  }

  void back() {
    if (_index == 0) {
      finish();
    } else {
      previous();
    }
  }

  void _goTo(int index) {
    _index = index;
    _targetBounds = null;
    notifyListeners();
  }

  void _reportBounds(Rect? bounds) {
    if (_disposed || bounds == _targetBounds) return;
    _targetBounds = bounds;
    if (_boundsNotificationScheduled) return;
    // The preview reports its painted position. Update the overlay next frame.
    _boundsNotificationScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _boundsNotificationScheduled = false;
      if (!_disposed) notifyListeners();
    });
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}

/// Measures a keyed control inside the preview, clipped to its scroll viewport.
class OnboardingTarget extends SingleChildRenderObjectWidget {
  const OnboardingTarget({
    super.key,
    required this.controller,
    required this.target,
    this.geometryChanges,
    required super.child,
  });

  final OnboardingController controller;
  final GlobalKey? target;
  final Listenable? geometryChanges;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _OnboardingTargetBox(controller, target, geometryChanges);

  @override
  void updateRenderObject(BuildContext context, RenderObject renderObject) {
    (renderObject as _OnboardingTargetBox).update(
      controller,
      target,
      geometryChanges,
    );
  }
}

class _OnboardingTargetBox extends RenderProxyBox {
  _OnboardingTargetBox(this._controller, this._target, this._geometryChanges);

  OnboardingController _controller;
  GlobalKey? _target;
  Listenable? _geometryChanges;

  void update(
    OnboardingController controller,
    GlobalKey? target,
    Listenable? geometryChanges,
  ) {
    if (_controller == controller &&
        _target == target &&
        _geometryChanges == geometryChanges) {
      return;
    }
    if (attached) _geometryChanges?.removeListener(markNeedsPaint);
    _controller = controller;
    _target = target;
    _geometryChanges = geometryChanges;
    if (attached) _geometryChanges?.addListener(markNeedsPaint);
    markNeedsPaint();
  }

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    _geometryChanges?.addListener(markNeedsPaint);
  }

  @override
  void detach() {
    _geometryChanges?.removeListener(markNeedsPaint);
    super.detach();
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    super.paint(context, offset);
    final box = _target?.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.attached || !box.hasSize) {
      _controller._reportBounds(null);
      return;
    }
    var bounds = MatrixUtils.transformRect(
      box.getTransformTo(null),
      Offset.zero & box.size,
    );
    final viewport = RenderAbstractViewport.maybeOf(box);
    if (viewport != null) {
      bounds = bounds.intersect(
        MatrixUtils.transformRect(
          viewport.getTransformTo(null),
          viewport.paintBounds,
        ),
      );
    }
    _controller._reportBounds(
      bounds.isEmpty || !bounds.isFinite ? null : bounds,
    );
  }
}
