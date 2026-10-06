import 'dart:ui' show lerpDouble;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

const double _recordingControlWidth = 208;
const double _recordingControlHeight = 52;
const double _recordingControlGlassBackgroundAlpha = 0.36;
const double _recordingControlGlassBorderAlpha = 0.62;
const double _recordingControlGlassBlurSigma = 28;
const double _recordingControlGlassReflectionAlpha = 0.2;
const double _recordingControlInnerAlpha = 0.55;
const double _recordingEndControlInnerAlpha = 0.8;

class RecordingButtons extends StatefulWidget {
  const RecordingButtons({super.key});

  @override
  State<RecordingButtons> createState() => _RecordingButtonsState();
}

class _RecordingButtonsState extends State<RecordingButtons> {
  Future<void> _showEndJourneyDialog() async {
    AppHaptics.warning();
    final gpsManager = context.read<GpsManager>();
    final shouldEndJourney = await showCommonDialog(
      context,
      context.tr('home.end_journey_message'),
      hasCancel: true,
      title: context.tr('home.end_journey_title'),
      confirmButtonText: context.tr('common.end'),
      confirmVariant: AppButtonVariant.danger,
    );

    if (shouldEndJourney) {
      gpsManager.changeRecordingState(GpsRecordingStatus.none);
    }
  }

  @override
  Widget build(BuildContext context) {
    final status = context.select<GpsManager, GpsRecordingStatus>(
      (gpsManager) => gpsManager.recordingStatus,
    );
    final gpsManager = context.read<GpsManager>();

    return PointerInterceptor(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 360),
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 280),
          switchInCurve: Curves.easeOutBack,
          switchOutCurve: Curves.easeInCubic,
          transitionBuilder: (child, animation) => FadeTransition(
            opacity: animation,
            child: ScaleTransition(
              scale: Tween<double>(begin: 0.94, end: 1).animate(animation),
              child: child,
            ),
          ),
          child: switch (status) {
            GpsRecordingStatus.none => _StartJourneyButton(
              key: const ValueKey('record-start'),
              onPressed: () {
                AppHaptics.heavy();
                gpsManager.changeRecordingState(GpsRecordingStatus.recording);
              },
            ),
            GpsRecordingStatus.recording => _ActiveJourneyControls(
              key: const ValueKey('record-active'),
              isPaused: false,
              onPrimaryPressed: () {
                AppHaptics.medium();
                gpsManager.changeRecordingState(GpsRecordingStatus.paused);
              },
              onEndPressed: _showEndJourneyDialog,
            ),
            GpsRecordingStatus.paused => _ActiveJourneyControls(
              key: const ValueKey('record-paused'),
              isPaused: true,
              onPrimaryPressed: () {
                AppHaptics.medium();
                gpsManager.changeRecordingState(GpsRecordingStatus.recording);
              },
              onEndPressed: _showEndJourneyDialog,
            ),
          },
        ),
      ),
    );
  }
}

class _StartJourneyButton extends StatelessWidget {
  const _StartJourneyButton({super.key, required this.onPressed});

  final VoidCallback onPressed;

  static const _iconGap = 7.0;

  @override
  Widget build(BuildContext context) {
    final label = context.tr('home.start_new_journey');
    final labelStyle = AppTypography.surfaceTitle.copyWith(
      color: context.appColors.onPrimaryActionColor,
      fontSize: context.locale.languageCode == 'en' ? 15 : null,
      fontWeight: FontWeight.w600,
    );

    return ConstrainedBox(
      constraints: const BoxConstraints(
        minWidth: _recordingControlWidth,
        minHeight: _recordingControlHeight,
        maxHeight: _recordingControlHeight,
      ),
      child: LiquidGlassSurface(
        borderRadius: BorderRadius.circular(23),
        backgroundAlpha:
            context.appColors._primaryRecordingGlassBackgroundAlpha,
        borderAlpha: context.appColors._primaryRecordingGlassBorderAlpha,
        blurSigma: context.appColors._primaryRecordingGlassBlurSigma,
        reflectionAlpha:
            context.appColors._primaryRecordingGlassReflectionAlpha,
        shadowAlpha: context.appColors._primaryRecordingShadowAlpha,
        child: Material(
          color: context.appColors.primaryGreen.withValues(
            alpha: context.appColors._primaryRecordingInnerAlpha,
          ),
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(23),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    Icons.play_arrow_rounded,
                    size: 21,
                    color: context.appColors.onPrimaryActionColor,
                  ),
                  const SizedBox(width: _iconGap),
                  Flexible(
                    child: Text(
                      label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: labelStyle,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ActiveJourneyControls extends StatelessWidget {
  const _ActiveJourneyControls({
    super.key,
    required this.isPaused,
    required this.onPrimaryPressed,
    required this.onEndPressed,
  });

  final bool isPaused;
  final VoidCallback onPrimaryPressed;
  final VoidCallback onEndPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: _recordingControlWidth,
      height: _recordingControlHeight,
      child: Center(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 112,
              height: 42,
              child: LiquidGlassSurface(
                borderRadius: BorderRadius.circular(18),
                backgroundAlpha: isPaused
                    ? context.appColors._primaryRecordingGlassBackgroundAlpha
                    : _recordingControlGlassBackgroundAlpha,
                borderAlpha: isPaused
                    ? context.appColors._primaryRecordingGlassBorderAlpha
                    : _recordingControlGlassBorderAlpha,
                blurSigma: isPaused
                    ? context.appColors._primaryRecordingGlassBlurSigma
                    : _recordingControlGlassBlurSigma,
                reflectionAlpha: isPaused
                    ? context.appColors._primaryRecordingGlassReflectionAlpha
                    : _recordingControlGlassReflectionAlpha,
                shadowAlpha: isPaused
                    ? context.appColors._primaryRecordingShadowAlpha
                    : null,
                child: isPaused
                    ? AppButton(
                        label: context.tr('home.resume'),
                        onPressed: onPrimaryPressed,
                        icon: Icons.play_arrow_rounded,
                        variant: AppButtonVariant.primary,
                        size: AppButtonSize.compact,
                        fontSize: 15,
                        backgroundAlpha:
                            context.appColors._primaryRecordingInnerAlpha,
                        borderRadius: 18,
                        expand: true,
                      )
                    : _PauseGlassButton(onPressed: onPrimaryPressed),
              ),
            ),
            const SizedBox(width: 6),
            SizedBox.square(
              dimension: 42,
              child: LiquidGlassSurface(
                circular: true,
                backgroundAlpha: _recordingControlGlassBackgroundAlpha,
                borderAlpha: _recordingControlGlassBorderAlpha,
                blurSigma: _recordingControlGlassBlurSigma,
                reflectionAlpha: _recordingControlGlassReflectionAlpha,
                reflectionColor: context.appColors.glassHighlightColor,
                secondaryReflectionColor: context.appColors.glassHighlightColor,
                child: AppIconButton(
                  onPressed: onEndPressed,
                  icon: Icons.stop_rounded,
                  variant: AppButtonVariant.danger,
                  size: 42,
                  backgroundAlpha: _recordingEndControlInnerAlpha,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PauseGlassButton extends StatelessWidget {
  const _PauseGlassButton({required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: context.appColors.surfaceColor.withValues(
        alpha: _recordingControlInnerAlpha,
      ),
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(18),
        child: Center(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.pause_rounded,
                size: 16,
                color: context.appColors.deepGreen,
              ),
              const SizedBox(width: 7),
              Text(
                context.tr('home.pause'),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppTypography.compactButton.copyWith(
                  color: context.appColors.deepGreen,
                  fontSize: 15,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// Start and resume share a glass treatment; the active controls keep theirs.
extension _PrimaryRecordingEffects on AppColors {
  double get _primaryRecordingGlassBackgroundAlpha =>
      lerpDouble(0.36, 0.1, darkProgress)!;
  double get _primaryRecordingGlassBorderAlpha =>
      lerpDouble(0.62, 0.28, darkProgress)!;
  double get _primaryRecordingGlassBlurSigma =>
      lerpDouble(28.0, 18.0, darkProgress)!;
  double get _primaryRecordingGlassReflectionAlpha =>
      lerpDouble(0.2, 0.06, darkProgress)!;
  double get _primaryRecordingInnerAlpha =>
      lerpDouble(0.55, 0.9, darkProgress)!;
  double get _primaryRecordingShadowAlpha =>
      lerpDouble(0.18, 0.3, darkProgress)!;
}
