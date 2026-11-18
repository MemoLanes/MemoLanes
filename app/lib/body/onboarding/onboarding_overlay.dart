import 'dart:math' as math;

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/layout_geometry.dart' as geometry_helpers;
import 'package:memolanes/body/onboarding/onboarding_controller.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

const _cardPadding = EdgeInsets.fromLTRB(14, 12, 14, 4);
const _actionGap = 2.0;
const _minCardWidth = 240.0;
const _minCardHeight = 144.0;

/// A transparent, input-blocking layer above the preview controls and map.
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
          if (controller.step == OnboardingStep.achievements ||
              controller.step == OnboardingStep.settings) {
            target = navigation;
          }
          final placement = _SpotlightGeometry(
            target: target,
            navigation: navigation,
            cardRegion: layout.regionFor(
              target?.center ?? layout.controlRegion.center,
            ),
            navigationBounds: nav,
            skipRegion: layout.regionFor(layout.safeBounds.topRight),
          );

          return BlockSemantics(
            child: PointerInterceptor(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () {},
                child: CustomPaint(
                  painter: _SpotlightPainter(placement),
                  child: CustomMultiChildLayout(
                    delegate: _CoachLayout(
                      placement,
                      allowCompactPlacement:
                          controller.step == OnboardingStep.journeys,
                    ),
                    children: [
                      LayoutId(
                        id: 'skip',
                        child: AppButton(
                          key: const ValueKey('onboarding-skip'),
                          label: context.tr('onboarding.skip'),
                          variant: AppButtonVariant.secondary,
                          borderRadius: 24,
                          fontSize: 13,
                          labelMaxLines: 2,
                          onPressed: controller.finish,
                        ),
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

class _GuideActions extends StatelessWidget {
  const _GuideActions({required this.controller});

  final OnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final last = controller.index == OnboardingStep.values.length - 1;
    // Keep the painted button compact while preserving the padded tap target.
    final buttonStyle = ButtonStyle(
      minimumSize: const WidgetStatePropertyAll(
        Size(kMinInteractiveDimension, 28),
      ),
      padding: const WidgetStatePropertyAll(
        EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      ),
      tapTargetSize: MaterialTapTargetSize.padded,
      visualDensity: VisualDensity.standard,
      textStyle: const WidgetStatePropertyAll(AppTypography.sectionLabel),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
      ),
    );
    return OverflowBar(
      alignment: MainAxisAlignment.spaceBetween,
      spacing: 6,
      overflowAlignment: OverflowBarAlignment.end,
      children: [
        Text(
          context.tr(
            'onboarding.progress',
            namedArgs: {
              'page': context.tr('onboarding.tabs.${controller.step.tab}'),
              'current': '${controller.index + 1}',
              'total': '${OnboardingStep.values.length}',
            },
          ),
          style: AppTypography.caption.copyWith(color: colors.mutedInkColor),
        ),
        Wrap(
          spacing: 6,
          alignment: WrapAlignment.end,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            TextButton(
              key: const ValueKey('onboarding-previous'),
              onPressed: controller.index == 0 ? null : controller.previous,
              style: TextButton.styleFrom(
                foregroundColor: colors.mutedInkColor,
                disabledForegroundColor: colors.subtleInkColor,
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
              ).merge(buttonStyle),
              child: Text(context.tr('onboarding.previous')),
            ),
            FilledButton(
              key: const ValueKey('onboarding-next'),
              onPressed: controller.next,
              style: FilledButton.styleFrom(
                backgroundColor: colors.selectedSurfaceColor,
                foregroundColor: colors.deepGreen,
              ).merge(buttonStyle),
              child: Text(
                context.tr(last ? 'onboarding.finish' : 'onboarding.next'),
              ),
            ),
          ],
        ),
      ],
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
    final copy = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Semantics(
          liveRegion: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            spacing: 5,
            children: [
              Text(
                context.tr('$prefix.title'),
                style: AppTypography.surfaceTitle.copyWith(
                  color: colors.deepGreen,
                ),
              ),
              Text(
                context.tr(targetReady ? '$prefix.body' : 'onboarding.waiting'),
                style: AppTypography.body.copyWith(color: colors.inkColor),
              ),
            ],
          ),
        ),
        if (controller.step == OnboardingStep.recording) ...[
          const SizedBox(height: 14),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            spacing: 12,
            children: [
              const RecIndicator(isRecording: true),
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
    );
    return Semantics(
      scopesRoute: true,
      namesRoute: true,
      explicitChildNodes: true,
      label: context.tr('onboarding.title'),
      child: AppDialogSurface(
        key: const ValueKey('onboarding-coach-card'),
        backgroundColor: colors.surfaceColor,
        borderRadius: const BorderRadius.all(Radius.circular(16)),
        child: Padding(
          padding: _cardPadding,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final scaler = MediaQuery.textScalerOf(context);
              final caption = AppTypography.caption;
              final captionHeight =
                  scaler.scale(caption.fontSize!) * caption.height!;
              // Leave room for up to two action rows and wrapping progress.
              final scrollWholeCard =
                  constraints.maxHeight <
                      2 * kMinInteractiveDimension +
                          3 * captionHeight +
                          _actionGap ||
                  constraints.maxWidth <
                      _minCardWidth - _cardPadding.horizontal ||
                  scaler.scale(AppTypography.sectionLabel.fontSize!) >
                      2 * AppTypography.sectionLabel.fontSize!;
              final content = Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                spacing: _actionGap,
                children: [
                  if (scrollWholeCard)
                    copy
                  else
                    Flexible(
                      child: SingleChildScrollView(primary: false, child: copy),
                    ),
                  _GuideActions(controller: controller),
                ],
              );
              return scrollWholeCard
                  ? SingleChildScrollView(primary: false, child: content)
                  : content;
            },
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
    required this.cardRegion,
    required this.navigationBounds,
    required this.skipRegion,
  });

  final Rect? target;
  final Rect? navigation;
  final Rect cardRegion;
  final Rect navigationBounds;
  final Rect skipRegion;
  Rect card = Rect.zero;
}

class _CoachLayout extends MultiChildLayoutDelegate {
  _CoachLayout(this.geometry, {required this.allowCompactPlacement});

  final _SpotlightGeometry geometry;
  final bool allowCompactPlacement;

  @override
  void performLayout(Size size) {
    final target = geometry.target?.inflate(36);
    final viewport = Offset.zero & size;
    final skipArea = geometryInset(geometry.skipRegion.intersect(viewport));
    final skipSize = layoutChild('skip', BoxConstraints.loose(skipArea.size));
    final skipOffset = skipArea.topRight - Offset(skipSize.width, 0);
    positionChild('skip', skipOffset);
    final region = geometryInset(geometry.cardRegion.intersect(viewport));
    final clearRegions = geometryClear(
      region.isEmpty ? geometryInset(viewport) : region,
      (skipOffset & skipSize).inflate(12),
    ).toList();
    final candidates = <Rect>[];
    for (final clear in clearRegions) {
      if (target == null || !clear.overlaps(target)) {
        candidates.add(clear);
        continue;
      }
      final cut = clear.intersect(target);
      candidates.addAll([
        Rect.fromLTRB(clear.left, clear.top, clear.right, cut.top),
        Rect.fromLTRB(clear.left, cut.bottom, clear.right, clear.bottom),
        Rect.fromLTRB(clear.left, clear.top, cut.left, clear.bottom),
        Rect.fromLTRB(cut.right, clear.top, clear.right, clear.bottom),
      ]);
    }
    final minWidth = math.min(_minCardWidth, size.width - 32);
    final usable = candidates
        .where((r) => r.width >= minWidth && r.height >= _minCardHeight)
        .toList();
    if (allowCompactPlacement && target != null && usable.isEmpty) {
      final minScrollableHeight =
          _cardPadding.vertical + kMinInteractiveDimension;
      usable.addAll(
        candidates.where(
          (r) => r.width >= minWidth && r.height >= minScrollableHeight,
        ),
      );
    }
    // The explanation can overlap a large target in a very short window,
    // but must always stay clear of the skip button.
    final choices = usable.isNotEmpty ? usable : clearRegions;
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

  Iterable<Rect> geometryClear(Rect region, Rect skip) sync* {
    final navigation = [
      if (!geometry.navigationBounds.isEmpty)
        geometry.navigationBounds.inflate(8),
    ];
    yield* geometry_helpers.clearRegions(region, [skip, ...navigation]);
    if (region.overlaps(skip) && skip.left > region.left) {
      // The card can use the full height to the left of the small skip button.
      yield* geometry_helpers.clearRegions(
        Rect.fromLTRB(region.left, region.top, skip.left, region.bottom),
        navigation,
      );
    }
  }

  double score(Rect rect) {
    // Count the space that can actually show text above the action row.
    final fixedHeight =
        _cardPadding.vertical + _actionGap + kMinInteractiveDimension;
    final copyHeight = math.min(rect.height, 280) - fixedHeight;
    return math.min(rect.width, 360) * math.max(1, copyHeight);
  }

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
