import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyLayerFilterMenu extends StatefulWidget {
  final Set<JourneyKind> selectedKinds;
  final ValueChanged<Set<JourneyKind>> onChanged;

  const JourneyLayerFilterMenu({
    super.key,
    required this.selectedKinds,
    required this.onChanged,
  });

  @override
  State<JourneyLayerFilterMenu> createState() => _JourneyLayerFilterMenuState();
}

class _JourneyLayerFilterMenuState extends State<JourneyLayerFilterMenu> {
  late Set<JourneyKind> _localKinds;

  @override
  void initState() {
    super.initState();
    _localKinds = Set.from(widget.selectedKinds);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 4),
          child: Text(
            context.tr('journey.list.filter_layers'),
            style: AppTypography.caption.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
        ),
        _buildItem(
          context,
          label: context.tr('journey_kind.default'),
          selected: _localKinds.contains(JourneyKind.defaultKind),
          onTap: () => _toggle(JourneyKind.defaultKind),
        ),
        _buildItem(
          context,
          label: context.tr('journey_kind.flight'),
          selected: _localKinds.contains(JourneyKind.flight),
          onTap: () => _toggle(JourneyKind.flight),
        ),
      ],
    );
  }

  void _toggle(JourneyKind kind) {
    final next = Set<JourneyKind>.from(_localKinds);
    if (next.contains(kind)) {
      if (next.length == 1) {
        AppHaptics.warning();
        Fluttertoast.showToast(
          msg: context.tr('journey.list.filter_at_least_one_layer'),
        );
        return;
      }
      next.remove(kind);
    } else {
      next.add(kind);
    }
    if (setEquals(next, _localKinds)) return;
    setState(() => _localKinds = next);
    widget.onChanged(next);
  }

  Widget _buildItem(
    BuildContext context, {
    required String label,
    required bool selected,
    required VoidCallback onTap,
  }) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              selected ? Icons.check : Icons.check_box_outline_blank,
              size: 18,
              color: selected
                  ? StyleConstants.deepGreen
                  : StyleConstants.mutedInkColor,
            ),
            const SizedBox(width: 8),
            Text(
              label,
              style: AppTypography.body.copyWith(
                color: selected
                    ? StyleConstants.inkColor
                    : StyleConstants.mutedInkColor,
                fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
