import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/frosted_bar_container.dart';
import 'package:memolanes/common/component/frosted_bar_item.dart';
import 'package:memolanes/common/component/frosted_bar_selection_group.dart';
import 'package:memolanes/body/journey/editor/journey_editor_overlay_layout.dart';

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
  static const double extent = JourneyEditorOverlayLayout.modeBarExtent;
  static const double modeItemExtent = 60.0;

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
        child: FrostedBarContainer(
          extent: extent,
          mainAxisPadding: 8,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              FrostedBarSelectionGroup(
                selectedIndex: _selectionIndexFor(currentMode),
                itemExtent: modeItemExtent,
                crossAxisExtent: extent,
                children: _buildModeItems(context),
              ),
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
    );
  }

  List<Widget> _buildModeItems(BuildContext context) {
    return [
      _ModeSwitchItem(
        mode: OperationMode.move,
        currentMode: currentMode,
        onModeChanged: onModeChanged,
        icon: Icons.open_with_rounded,
        label: context.tr('journey.editor.move'),
      ),
      _buildDrawModeItem(context),
      _ModeSwitchItem(
        mode: OperationMode.delete,
        currentMode: currentMode,
        onModeChanged: onModeChanged,
        icon: Icons.delete,
        label: context.tr('journey.editor.erase'),
      ),
    ];
  }

  int _selectionIndexFor(OperationMode mode) {
    return switch (mode) {
      OperationMode.move => 0,
      OperationMode.edit || OperationMode.editReadonly => 1,
      OperationMode.delete => 2,
    };
  }

  Widget _buildDrawModeItem(BuildContext context) {
    return FrostedBarItem(
      icon: Icons.gesture_rounded,
      label: context.tr('journey.editor.draw'),
      isSelected: currentMode == OperationMode.edit ||
          currentMode == OperationMode.editReadonly,
      showSelectionBackground: false,
      onTap: () {
        AppHaptics.light();
        onModeChanged(OperationMode.edit);
      },
    );
  }

  Widget _buildActionButton({
    required IconData icon,
    required String label,
    required bool isEnabled,
    VoidCallback? onTap,
  }) {
    return FrostedBarItem(
      icon: icon,
      label: label,
      isEnabled: isEnabled,
      isSelected: isEnabled,
      showSelectionBackground: false,
      onTap: isEnabled
          ? () {
              AppHaptics.medium();
              onTap?.call();
            }
          : null,
    );
  }
}

class _ModeSwitchItem extends StatelessWidget {
  final OperationMode mode;
  final OperationMode currentMode;
  final ValueChanged<OperationMode> onModeChanged;
  final IconData icon;
  final String label;

  const _ModeSwitchItem({
    required this.mode,
    required this.currentMode,
    required this.onModeChanged,
    required this.icon,
    required this.label,
  });

  @override
  Widget build(BuildContext context) {
    final selected = currentMode == mode;

    return FrostedBarItem(
      icon: icon,
      label: label,
      isSelected: selected,
      showSelectionBackground: false,
      onTap: () {
        AppHaptics.light();
        onModeChanged(mode);
      },
    );
  }
}
