import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum OperationMode { move, edit, editReadonly, delete }

enum DrawEntryMode { freehand, linked }

class ModeSwitchBar extends StatelessWidget {
  static const double extent = 56.0;
  static const double safeAreaMinimum = 16.0;

  final OperationMode currentMode;
  final ValueChanged<OperationMode> onModeChanged;
  final DrawEntryMode currentDrawMode;
  final bool isDrawMenuOpen;
  final VoidCallback onDrawPressed;
  final ValueChanged<DrawEntryMode> onDrawModeChanged;
  final VoidCallback? onUndo;
  final VoidCallback? onSave;

  const ModeSwitchBar({
    super.key,
    required this.currentMode,
    required this.onModeChanged,
    required this.currentDrawMode,
    required this.isDrawMenuOpen,
    required this.onDrawPressed,
    required this.onDrawModeChanged,
    this.onUndo,
    this.onSave,
  });

  bool get _isDrawSelected =>
      currentMode == OperationMode.edit ||
      currentMode == OperationMode.editReadonly;

  @override
  Widget build(BuildContext context) {
    final selectedModeWidth = MediaQuery.sizeOf(context).width < 340
        ? 58.0
        : 66.0;
    final saveCallback = onSave;

    return Align(
      alignment: Alignment.bottomCenter,
      child: IntrinsicWidth(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 190),
              reverseDuration: const Duration(milliseconds: 150),
              switchInCurve: Curves.easeOutCubic,
              switchOutCurve: Curves.easeInCubic,
              transitionBuilder: (child, animation) {
                return FadeTransition(
                  opacity: animation,
                  child: SizeTransition(
                    sizeFactor: animation,
                    alignment: Alignment.bottomCenter,
                    child: child,
                  ),
                );
              },
              child: isDrawMenuOpen
                  ? Padding(
                      key: const ValueKey('draw-mode-menu'),
                      padding: const EdgeInsets.only(left: 2, bottom: 8),
                      child: _DrawModeMenu(
                        currentMode: currentDrawMode,
                        onModeChanged: onDrawModeChanged,
                      ),
                    )
                  : const SizedBox.shrink(
                      key: ValueKey('draw-mode-menu-hidden'),
                    ),
            ),
            LiquidGlassSurface(
              borderRadius: BorderRadius.circular(22),
              backgroundAlpha: 0.4,
              borderAlpha: 0.42,
              blurSigma: 28,
              reflectionAlpha: 0.18,
              padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 6),
              child: SizedBox(
                height: extent - 12,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    _EditorModeButton(
                      icon: Icons.open_with_rounded,
                      label: context.tr('journey.editor.move'),
                      isSelected: currentMode == OperationMode.move,
                      selectedWidth: selectedModeWidth,
                      onTap: () {
                        AppHaptics.selection();
                        onModeChanged(OperationMode.move);
                      },
                    ),
                    _EditorModeButton(
                      icon: Icons.gesture_rounded,
                      label: context.tr('journey.editor.draw'),
                      isSelected: _isDrawSelected,
                      selectedWidth: selectedModeWidth,
                      onTap: () {
                        AppHaptics.selection();
                        onDrawPressed();
                      },
                    ),
                    _EditorModeButton(
                      icon: Icons.delete_sweep_rounded,
                      label: context.tr('journey.editor.erase'),
                      isSelected: currentMode == OperationMode.delete,
                      selectedWidth: selectedModeWidth,
                      isCaution: true,
                      onTap: () {
                        AppHaptics.selection();
                        onModeChanged(OperationMode.delete);
                      },
                    ),
                    Container(
                      width: 1,
                      height: 24,
                      margin: const EdgeInsets.symmetric(horizontal: 4),
                      color: StyleConstants.lineColor.withValues(alpha: 0.9),
                    ),
                    _UndoButton(
                      label: context.tr('journey.editor.undo'),
                      onTap: onUndo,
                    ),
                    const SizedBox(width: 3),
                    SizedBox(
                      width: 76,
                      child: AppButton(
                        label: context.tr('journey.editor.save'),
                        icon: Icons.check_rounded,
                        size: AppButtonSize.compact,
                        expand: true,
                        onPressed: saveCallback == null
                            ? null
                            : () {
                                AppHaptics.medium();
                                saveCallback();
                              },
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EditorModeButton extends StatelessWidget {
  const _EditorModeButton({
    required this.icon,
    required this.label,
    required this.isSelected,
    required this.selectedWidth,
    required this.onTap,
    this.isCaution = false,
  });

  final IconData icon;
  final String label;
  final bool isSelected;
  final double selectedWidth;
  final VoidCallback onTap;
  final bool isCaution;

  @override
  Widget build(BuildContext context) {
    final activeColor = isCaution
        ? StyleConstants.warningInkColor
        : StyleConstants.deepGreen;
    final activeBackground = isCaution
        ? StyleConstants.warningColor.withValues(alpha: 0.28)
        : StyleConstants.primaryGreen.withValues(alpha: 0.25);

    return Semantics(
      selected: isSelected,
      button: true,
      label: label,
      child: Tooltip(
        message: label,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 220),
          curve: Curves.easeOutCubic,
          width: isSelected ? selectedWidth : 44,
          height: 44,
          decoration: BoxDecoration(
            color: isSelected ? activeBackground : Colors.transparent,
            borderRadius: BorderRadius.circular(16),
          ),
          child: Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(16),
              child: Center(
                child: TweenAnimationBuilder<double>(
                  tween: Tween(begin: 0, end: isSelected ? 1 : 0),
                  duration: const Duration(milliseconds: 220),
                  curve: Curves.easeInOutCubic,
                  builder: (context, progress, _) {
                    final focusTransition = 4 * progress * (1 - progress);
                    final color = Color.lerp(
                      StyleConstants.mutedInkColor,
                      activeColor,
                      progress,
                    );

                    return Transform.scale(
                      scale: 1 + progress * 0.035,
                      child: ImageFiltered(
                        imageFilter: ImageFilter.blur(
                          sigmaX: focusTransition * 1.15,
                          sigmaY: focusTransition * 1.15,
                          tileMode: TileMode.decal,
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(icon, size: 22, color: color),
                            if (isSelected) ...[
                              const SizedBox(width: 4),
                              Flexible(
                                child: Text(
                                  label,
                                  maxLines: 1,
                                  overflow: TextOverflow.fade,
                                  softWrap: false,
                                  style: AppTypography.micro.copyWith(
                                    color: activeColor,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _UndoButton extends StatelessWidget {
  const _UndoButton({required this.label, required this.onTap});

  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final tapCallback = onTap;

    return Semantics(
      button: true,
      enabled: tapCallback != null,
      label: label,
      child: Tooltip(
        message: label,
        child: SizedBox.square(
          dimension: 42,
          child: IconButton(
            onPressed: tapCallback == null
                ? null
                : () {
                    AppHaptics.light();
                    tapCallback();
                  },
            style: IconButton.styleFrom(
              backgroundColor: StyleConstants.surfaceColor.withValues(
                alpha: 0.56,
              ),
              foregroundColor: StyleConstants.deepGreen,
              disabledBackgroundColor: StyleConstants.surfaceColor.withValues(
                alpha: 0.24,
              ),
              disabledForegroundColor: StyleConstants.mutedInkColor.withValues(
                alpha: 0.38,
              ),
              side: BorderSide(
                color: StyleConstants.lineColor.withValues(alpha: 0.82),
              ),
            ),
            icon: const Icon(Icons.undo_rounded, size: 20),
          ),
        ),
      ),
    );
  }
}

class _DrawModeMenu extends StatelessWidget {
  const _DrawModeMenu({required this.currentMode, required this.onModeChanged});

  final DrawEntryMode currentMode;
  final ValueChanged<DrawEntryMode> onModeChanged;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 156,
      child: LiquidGlassSurface(
        borderRadius: BorderRadius.circular(18),
        backgroundAlpha: 0.52,
        borderAlpha: 0.48,
        blurSigma: 28,
        reflectionAlpha: 0.14,
        padding: const EdgeInsets.all(6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _DrawModeOption(
              icon: Icons.draw_rounded,
              label: context.tr('journey.editor.free_draw'),
              isSelected: currentMode == DrawEntryMode.freehand,
              onTap: () {
                AppHaptics.selection();
                onModeChanged(DrawEntryMode.freehand);
              },
            ),
            const SizedBox(height: 3),
            _DrawModeOption(
              icon: Icons.link_rounded,
              label: context.tr('journey.editor.linked_draw'),
              isSelected: currentMode == DrawEntryMode.linked,
              onTap: () {
                AppHaptics.selection();
                onModeChanged(DrawEntryMode.linked);
              },
            ),
          ],
        ),
      ),
    );
  }
}

class _DrawModeOption extends StatelessWidget {
  const _DrawModeOption({
    required this.icon,
    required this.label,
    required this.isSelected,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final contentColor = isSelected
        ? StyleConstants.deepGreen
        : StyleConstants.mutedInkColor;

    return Semantics(
      selected: isSelected,
      button: true,
      label: label,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOutCubic,
        height: 40,
        decoration: BoxDecoration(
          color: isSelected
              ? StyleConstants.primaryGreen.withValues(alpha: 0.25)
              : Colors.transparent,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(12),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 11),
              child: Row(
                children: [
                  Icon(icon, color: contentColor, size: 19),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppTypography.label.copyWith(
                        color: contentColor,
                        fontWeight: isSelected
                            ? FontWeight.w700
                            : FontWeight.w600,
                      ),
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
