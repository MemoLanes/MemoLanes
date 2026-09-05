import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/journey_header.dart';

String importPreprocessorLabel(
  BuildContext context,
  import_api.ImportPreprocessor value,
) => switch (value) {
  import_api.ImportPreprocessor.none => context.tr('import.preprocessor.none'),
  import_api.ImportPreprocessor.generic => context.tr(
    'import.preprocessor.generic',
  ),
  import_api.ImportPreprocessor.flightTrack => context.tr(
    'import.preprocessor.flight_track',
  ),
  import_api.ImportPreprocessor.spare => context.tr(
    'import.preprocessor.spare',
  ),
};

String journeyKindLabel(BuildContext context, JourneyKind value) =>
    switch (value) {
      JourneyKind.defaultKind => context.tr('journey_kind.default'),
      JourneyKind.flight => context.tr('journey_kind.flight'),
    };

class ImportPreprocessorTile extends StatelessWidget {
  const ImportPreprocessorTile({
    super.key,
    required this.value,
    required this.onSelected,
    this.onInfoTap,
    this.position = LabelTilePosition.single,
    this.bottom = true,
  });

  final import_api.ImportPreprocessor value;
  final ValueChanged<import_api.ImportPreprocessor> onSelected;
  final VoidCallback? onInfoTap;
  final LabelTilePosition position;
  final bool bottom;

  @override
  Widget build(BuildContext context) {
    return LabelTile(
      label: context.tr('import.preprocessor.label'),
      infoLabelOnTap: onInfoTap,
      position: position,
      bottom: bottom,
      trailing: LabelTileContent(
        content: importPreprocessorLabel(context, value),
        showArrow: true,
      ),
      onTap: () => _showOptionPicker(
        context,
        values: import_api.ImportPreprocessor.values,
        selectedValue: value,
        labelOf: (item) => importPreprocessorLabel(context, item),
        onSelected: onSelected,
      ),
    );
  }
}

class JourneyKindTile extends StatelessWidget {
  const JourneyKindTile({
    super.key,
    required this.value,
    required this.onSelected,
    this.position = LabelTilePosition.single,
    this.bottom = true,
  });

  final JourneyKind value;
  final ValueChanged<JourneyKind> onSelected;
  final LabelTilePosition position;
  final bool bottom;

  @override
  Widget build(BuildContext context) {
    return LabelTile(
      label: context.tr('journey.journey_kind'),
      position: position,
      bottom: bottom,
      trailing: LabelTileContent(
        content: journeyKindLabel(context, value),
        showArrow: true,
      ),
      onTap: () => _showOptionPicker(
        context,
        values: JourneyKind.values,
        selectedValue: value,
        labelOf: (item) => journeyKindLabel(context, item),
        iconBuilder: (item) => JourneyKindIcon(
          kind: item,
          color: StyleConstants.deepGreen,
          size: 20,
        ),
        onSelected: onSelected,
      ),
    );
  }
}

class JourneyNoteTile extends StatelessWidget {
  const JourneyNoteTile({
    super.key,
    required this.controller,
    this.maxHeight = 150,
    this.maxLines = 5,
    this.widthFactor = 0.6,
    this.position = LabelTilePosition.single,
    this.bottom = true,
  });

  final TextEditingController controller;
  final double maxHeight;
  final int maxLines;
  final double widthFactor;
  final LabelTilePosition position;
  final bool bottom;

  @override
  Widget build(BuildContext context) {
    return LabelTile(
      label: context.tr('journey.note'),
      position: position,
      bottom: bottom,
      maxHeight: maxHeight,
      trailing: SizedBox(
        width: MediaQuery.sizeOf(context).width * widthFactor,
        child: TextField(
          controller: controller,
          onTapOutside: (_) => FocusManager.instance.primaryFocus?.unfocus(),
          keyboardType: TextInputType.multiline,
          textInputAction: TextInputAction.newline,
          maxLines: maxLines,
          minLines: 1,
          style: AppTypography.body.copyWith(color: StyleConstants.inkColor),
          decoration: InputDecoration.collapsed(
            border: InputBorder.none,
            hintText: context.tr('common.please_enter'),
            hintStyle: AppTypography.body.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
          textAlign: TextAlign.right,
        ),
      ),
    );
  }
}

void _showOptionPicker<T>(
  BuildContext context, {
  required List<T> values,
  required T selectedValue,
  required String Function(T value) labelOf,
  Widget Function(T value)? iconBuilder,
  required ValueChanged<T> onSelected,
}) {
  showBasicCard(
    context,
    builder: (dialogContext) => Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 0; i < values.length; i++) ...[
          if (i > 0) const SizedBox(height: 8),
          AppOptionTile(
            iconWidget: iconBuilder?.call(values[i]),
            title: labelOf(values[i]),
            selected: values[i] == selectedValue,
            trailing: AppOptionTileTrailing.selection,
            onTap: () {
              Navigator.of(dialogContext).pop();
              onSelected(values[i]);
            },
          ),
        ],
      ],
    ),
  );
}
