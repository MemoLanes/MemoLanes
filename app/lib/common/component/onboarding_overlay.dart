import 'dart:math' as math;

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/layout_geometry.dart' as geometry_helpers;
import 'package:memolanes/common/onboarding.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// A transparent, input-blocking layer above the real app, including its WebView.
class OnboardingOverlay extends StatefulWidget {
  const OnboardingOverlay({super.key, required this.controller});

  final OnboardingController controller;

  @override
  State<OnboardingOverlay> createState() => _OnboardingOverlayState();
}

class _OnboardingOverlayState extends State<OnboardingOverlay> {
  OnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    // The overlay itself needs one layout before converting global targets.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) setState(() {});
    });
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) {
      if (!controller.isActive) return const SizedBox.shrink();
      final layout = AppLayoutScope.of(context);
      return LayoutBuilder(
        builder: (context, constraints) {
          final box = context.findRenderObject() as RenderBox?;
          final globalTarget = controller.targetBounds;
          Rect? target;
          if (box != null && box.hasSize && globalTarget != null) {
            final local = Rect.fromPoints(
              box.globalToLocal(globalTarget.topLeft),
              box.globalToLocal(globalTarget.bottomRight),
            ).intersect(layout.safeBounds);
            if (!local.isEmpty) target = local.inflate(6);
          }

          final nav = layout.navigationBounds;
          final vertical = layout.navigationAxis == Axis.vertical;
          final navigation = nav.isEmpty
              ? null
              : Rect.fromLTWH(
                  nav.left +
                      (vertical ? 0 : nav.width / 5 * controller.step.tab),
                  nav.top +
                      (vertical ? nav.height / 5 * controller.step.tab : 0),
                  vertical ? nav.width : nav.width / 5,
                  vertical ? nav.height / 5 : nav.height,
                );
          // Introduce these whole pages through their navigation entries.
          if (controller.step.anchor == OnboardingAnchor.achievements ||
              controller.step.anchor == OnboardingAnchor.settings) {
            target = navigation;
          }
          final placement = _SpotlightGeometry(
            target: target,
            navigation: navigation,
            regions: [
              layout.regionFor(target?.center ?? layout.controlRegion.center),
            ],
            navigationBounds: nav,
            toolbarRegion: layout.regionFor(layout.safeBounds.topCenter),
          );

          return BlockSemantics(
            child: PointerInterceptor(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () {},
                child: CustomPaint(
                  painter: _SpotlightPainter(placement),
                  child: CustomMultiChildLayout(
                    delegate: _CoachLayout(placement),
                    children: [
                      LayoutId(
                        id: 'toolbar',
                        child: _GuideToolbar(controller: controller),
                      ),
                      LayoutId(
                        id: 'card',
                        child: _CoachCard(
                          key: ValueKey(controller.index),
                          controller: controller,
                          targetReady: target != null,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        },
      );
    },
  );
}

class _GuideToolbar extends StatelessWidget {
  const _GuideToolbar({required this.controller});

  final OnboardingController controller;

  @override
  Widget build(BuildContext context) {
    // Reserve two text lines even on steps with shorter labels. All three
    // controls keep the same position and size throughout the tour.
    final height = math.max(
      44.0,
      MediaQuery.textScalerOf(context).scale(12) * 1.15 * 2 + 16,
    );
    final last = controller.index == OnboardingController.steps.length - 1;
    return Material(
      key: const ValueKey('onboarding-toolbar'),
      color: context.appColors.canvasColor,
      borderRadius: BorderRadius.circular(20),
      elevation: 8,
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: SizedBox(
          height: height,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            spacing: 8,
            children: [
              Expanded(
                child: AppButton(
                  key: const ValueKey('onboarding-skip'),
                  label: context.tr('onboarding.skip'),
                  size: AppButtonSize.compact,
                  variant: AppButtonVariant.secondary,
                  labelMaxLines: 2,
                  onPressed: controller.finish,
                ),
              ),
              Expanded(
                child: AppButton(
                  key: const ValueKey('onboarding-previous'),
                  label: context.tr('onboarding.previous'),
                  size: AppButtonSize.compact,
                  variant: AppButtonVariant.secondary,
                  labelMaxLines: 2,
                  onPressed: controller.index == 0 ? null : controller.previous,
                ),
              ),
              Expanded(
                child: AppButton(
                  key: const ValueKey('onboarding-next'),
                  label: context.tr(
                    last ? 'onboarding.finish' : 'onboarding.next',
                  ),
                  size: AppButtonSize.compact,
                  labelMaxLines: 2,
                  onPressed: controller.next,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CoachCard extends StatelessWidget {
  const _CoachCard({
    super.key,
    required this.controller,
    required this.targetReady,
  });

  final OnboardingController controller;
  final bool targetReady;

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final prefix = 'onboarding.${controller.step.translationKey}';
    return Semantics(
      scopesRoute: true,
      namesRoute: true,
      explicitChildNodes: true,
      label: context.tr('onboarding.title'),
      child: Material(
        key: const ValueKey('onboarding-coach-card'),
        color: colors.canvasColor,
        elevation: 8,
        shadowColor: colors.shadowColor,
        borderRadius: BorderRadius.circular(22),
        clipBehavior: Clip.antiAlias,
        child: SingleChildScrollView(
          primary: false,
          padding: const EdgeInsets.all(18),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                context.tr(
                  'onboarding.progress',
                  namedArgs: {
                    'page': context.tr(
                      'onboarding.tabs.${controller.step.tab}',
                    ),
                    'current': '${controller.index + 1}',
                    'total': '${OnboardingController.steps.length}',
                  },
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppTypography.caption.copyWith(
                  color: colors.mutedInkColor,
                ),
              ),
              const SizedBox(height: 12),
              Semantics(
                liveRegion: true,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      context.tr('$prefix.title'),
                      style: AppTypography.surfaceTitle.copyWith(
                        color: colors.deepGreen,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      context.tr(
                        targetReady ? '$prefix.body' : 'onboarding.waiting',
                      ),
                      style: AppTypography.body.copyWith(
                        color: colors.inkColor,
                      ),
                    ),
                  ],
                ),
              ),
              if (controller.step.anchor == OnboardingAnchor.recording) ...[
                const SizedBox(height: 14),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const RecIndicator(isRecording: true),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        context.tr('onboarding.recording.rec_hint'),
                        style: AppTypography.supporting.copyWith(
                          color: colors.mutedInkColor,
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

// Shared by layout and paint so the arrow starts at the measured card edge,
// rather than assuming a text height or a particular device size.
class _SpotlightGeometry {
  _SpotlightGeometry({
    required this.target,
    required this.navigation,
    required this.regions,
    required this.navigationBounds,
    required this.toolbarRegion,
  });

  final Rect? target;
  final Rect? navigation;
  final List<Rect> regions;
  final Rect navigationBounds;
  final Rect toolbarRegion;
  Rect toolbar = Rect.zero;
  Rect card = Rect.zero;
}

class _CoachLayout extends MultiChildLayoutDelegate {
  _CoachLayout(this.geometry);

  final _SpotlightGeometry geometry;

  @override
  void performLayout(Size size) {
    final target = geometry.target;
    final toolbarArea = geometryInset(
      geometry.toolbarRegion.intersect(Offset.zero & size),
    );
    final toolbarSize = layoutChild(
      'toolbar',
      BoxConstraints.tightFor(width: math.min(520, toolbarArea.width)),
    );
    geometry.toolbar = Rect.fromLTWH(
      toolbarArea.center.dx - toolbarSize.width / 2,
      toolbarArea.top,
      toolbarSize.width,
      toolbarSize.height,
    );
    positionChild('toolbar', geometry.toolbar.topLeft);
    final safeRegions = [
      for (final region in geometry.regions)
        geometryInset(region.intersect(Offset.zero & size)),
    ].where((r) => !r.isEmpty).toList();
    if (safeRegions.isEmpty) {
      safeRegions.add(geometryInset(Offset.zero & size));
    }
    final candidates = <Rect>[];
    for (final region in safeRegions) {
      for (final clear in geometryClear(region)) {
        if (target == null || !clear.overlaps(target.inflate(36))) {
          candidates.add(clear);
          continue;
        }
        final cut = clear.intersect(target.inflate(36));
        candidates.addAll([
          Rect.fromLTRB(clear.left, clear.top, clear.right, cut.top),
          Rect.fromLTRB(clear.left, cut.bottom, clear.right, clear.bottom),
          Rect.fromLTRB(clear.left, clear.top, cut.left, clear.bottom),
          Rect.fromLTRB(cut.right, clear.top, clear.right, clear.bottom),
        ]);
      }
    }
    final usable = candidates
        .where(
          (r) => r.width >= math.min(240, size.width - 32) && r.height >= 100,
        )
        .toList();
    // The explanation can overlap a large target in a very short window,
    // but must always stay clear of the fixed toolbar.
    final choices = usable.isNotEmpty
        ? usable
        : [for (final region in safeRegions) ...geometryClear(region)];
    final area = choices.reduce((a, b) => score(b) > score(a) ? b : a);
    final cardSize = layoutChild(
      'card',
      BoxConstraints(
        minWidth: math.min(360, area.width),
        maxWidth: math.min(360, area.width),
        maxHeight: area.height,
      ),
    );
    final focus = target?.center ?? area.center;
    final left = (focus.dx - cardSize.width / 2).clamp(
      area.left,
      area.right - cardSize.width,
    );
    final top = (focus.dy - cardSize.height / 2).clamp(
      area.top,
      area.bottom - cardSize.height,
    );
    geometry.card = Offset(left, top) & cardSize;
    positionChild('card', geometry.card.topLeft);
  }

  Rect geometryInset(Rect region) =>
      geometry_helpers.insetRect(region, const EdgeInsets.all(16));

  List<Rect> geometryClear(Rect region) =>
      geometry_helpers.clearRegions(region, [
        geometry.toolbar.inflate(12),
        if (!geometry.navigationBounds.isEmpty)
          geometry.navigationBounds.inflate(8),
      ]);

  double score(Rect rect) =>
      math.min(rect.width, 360) * math.min(rect.height, 280);

  @override
  bool shouldRelayout(_CoachLayout oldDelegate) => true;
}

class _SpotlightPainter extends CustomPainter {
  _SpotlightPainter(this.geometry);

  final _SpotlightGeometry geometry;

  @override
  void paint(Canvas canvas, Size size) {
    final holes = <RRect>[
      if (geometry.target case final target?)
        RRect.fromRectAndRadius(target, const Radius.circular(20)),
      if (geometry.navigation case final nav? when nav != geometry.target)
        RRect.fromRectAndRadius(nav, const Radius.circular(20)),
    ];
    var mask = Path()..addRect(Offset.zero & size);
    for (final hole in holes) {
      mask = Path.combine(
        PathOperation.difference,
        mask,
        Path()..addRRect(hole),
      );
    }
    canvas.drawPath(mask, Paint()..color = const Color(0xB3000000));
    final stroke = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;
    for (final hole in holes) {
      canvas.drawRRect(hole, stroke);
    }

    final target = geometry.target;
    final card = geometry.card;
    if (target == null || card.isEmpty || card.overlaps(target)) return;
    final start = _edgeToward(card.inflate(7), target.center);
    final end = _edgeToward(target.inflate(7), card.center);
    if ((end - start).distance < 12) return;
    canvas.drawLine(start, end, stroke);
    final angle = math.atan2(end.dy - start.dy, end.dx - start.dx);
    for (final delta in [-math.pi / 6, math.pi / 6]) {
      canvas.drawLine(
        end,
        end - Offset(math.cos(angle + delta), math.sin(angle + delta)) * 9,
        stroke,
      );
    }
  }

  Offset _edgeToward(Rect rect, Offset point) {
    final delta = point - rect.center;
    final scale = math.min(
      delta.dx == 0 ? double.infinity : rect.width / 2 / delta.dx.abs(),
      delta.dy == 0 ? double.infinity : rect.height / 2 / delta.dy.abs(),
    );
    return rect.center + delta * scale;
  }

  @override
  bool shouldRepaint(_SpotlightPainter oldDelegate) => true;
}
