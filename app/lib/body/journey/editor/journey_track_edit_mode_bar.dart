import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

enum OperationMode {
  move,
  edit,
  editReadonly,
  delete,
}

enum DrawEntryMode {
  freehand,
  linked,
}

class ModeSwitchBar extends StatelessWidget {
  final OperationMode currentMode;
  final ValueChanged<OperationMode> onModeChanged;
  final bool canUndo;
  final VoidCallback? onUndo;
  final bool canSave;
  final VoidCallback? onSave;

  const ModeSwitchBar({
    super.key,
    required this.currentMode,
    required this.onModeChanged,
    this.canUndo = false,
    this.onUndo,
    this.canSave = false,
    this.onSave,
  });

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.bottomCenter,
      child: IntrinsicWidth(
        child: ClipRRect(
          borderRadius: BorderRadius.circular(16),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
            child: Container(
              height: 64,
              padding: const EdgeInsets.symmetric(horizontal: 8),
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.7),
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: Colors.white.withValues(alpha: 0.4)),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.08),
                    blurRadius: 20,
                    offset: const Offset(0, 4),
                  ),
                ],
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  ..._buildModeItems(context),
                  Container(
                    width: 1,
                    height: 24,
                    margin: const EdgeInsets.symmetric(horizontal: 8),
                    color: Colors.black12,
                  ),
                  _buildActionButton(
                    icon: Icons.undo_rounded,
                    label: context.tr('journey.editor.undo'),
                    isEnabled: canUndo,
                    onTap: onUndo,
                  ),
                  _buildActionButton(
                    icon: Icons.save,
                    label: context.tr('journey.editor.save'),
                    isEnabled: canSave,
                    onTap: onSave,
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  List<Widget> _buildModeItems(BuildContext context) {
    return [
      _buildModeItem(
        mode: OperationMode.move,
        icon: Icons.open_with_rounded,
        label: context.tr('journey.editor.move'),
      ),
      _buildDrawModeItem(context),
      _buildModeItem(
        mode: OperationMode.delete,
        icon: Icons.delete,
        label: context.tr('journey.editor.erase'),
      ),
    ];
  }

  Widget _buildDrawModeItem(BuildContext context) {
    return _BaseBarItem(
      icon: Icons.gesture_rounded,
      label: context.tr('journey.editor.draw'),
      isSelected: currentMode == OperationMode.edit ||
          currentMode == OperationMode.editReadonly,
      onTap: () {
        HapticFeedback.lightImpact();
        onModeChanged(OperationMode.edit);
      },
    );
  }

  Widget _buildModeItem({
    required OperationMode mode,
    required IconData icon,
    required String label,
    bool isEnabled = true,
    bool? isSelected,
  }) {
    final selected = isSelected ?? currentMode == mode;

    return _BaseBarItem(
      icon: icon,
      label: label,
      isSelected: selected,
      isEnabled: isEnabled,
      onTap: isEnabled
          ? () {
              HapticFeedback.lightImpact();
              onModeChanged(mode);
            }
          : null,
    );
  }

  Widget _buildActionButton({
    required IconData icon,
    required String label,
    required bool isEnabled,
    VoidCallback? onTap,
  }) {
    return _BaseBarItem(
      icon: icon,
      label: label,
      isEnabled: isEnabled,
      onTap: isEnabled
          ? () {
              HapticFeedback.mediumImpact();
              onTap?.call();
            }
          : null,
    );
  }
}

class _BaseBarItem extends StatelessWidget {
  final IconData icon;
  final String label;
  final bool isSelected;
  final bool isEnabled;
  final VoidCallback? onTap;

  const _BaseBarItem({
    required this.icon,
    required this.label,
    this.isSelected = false,
    this.isEnabled = true,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    const themeColor = Colors.black;

    final Color bgColor = isSelected
        ? (isEnabled
            ? themeColor.withValues(alpha: 0.12)
            : Colors.black.withValues(alpha: 0.05))
        : Colors.transparent;

    final Color contentColor = !isEnabled
        ? Colors.grey.shade400
        : isSelected
            ? themeColor
            : Colors.grey.shade800;

    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 250),
        margin: const EdgeInsets.symmetric(vertical: 6, horizontal: 2),
        padding: const EdgeInsets.symmetric(horizontal: 14),
        decoration: BoxDecoration(
          color: bgColor,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            SizedBox(
              width: 24,
              height: 24,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  Align(
                    alignment: Alignment.center,
                    child: Icon(icon, color: contentColor, size: 22),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 2),
            Text(
              label,
              style: TextStyle(
                fontSize: 10,
                fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
                color: contentColor,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
