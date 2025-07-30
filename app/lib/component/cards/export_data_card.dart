import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/component/cards/card_label_tile.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';
import 'package:memolanes/journey_info.dart';
import 'package:memolanes/src/rust/journey_header.dart';

// TODO: {export_data, import_data, journey_kind}_card are too similar, consider refactoring
class ExportDataCard extends StatelessWidget {
  const ExportDataCard({
    super.key,
    this.journeyType,
    this.onLabelTaped,
  });

  final JourneyType? journeyType;
  final Function(ExportType)? onLabelTaped;

  @override
  Widget build(BuildContext context) {
    return SafeAreaWrapper(
      child: Container(
        decoration: BoxDecoration(
          color: const Color(0x1AFFFFFF),
          borderRadius: BorderRadius.circular(16.0),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            CardLabelTile(
              position: journeyType != JourneyType.bitmap
                  ? CardLabelTilePosition.top
                  : CardLabelTilePosition.single,
              label: context.tr("journey.export_journey_as_mldx"),
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call(ExportType.mldx);
              },
              top: false,
            ),
            if (journeyType != JourneyType.bitmap) ...[
              Container(
                height: 0.5,
                color: const Color(0xFF262626),
              ),
              CardLabelTile(
                position: CardLabelTilePosition.middle,
                label: context.tr("journey.export_journey_as_kml"),
                onTap: () {
                  Navigator.pop(context);
                  onLabelTaped?.call(ExportType.kml);
                },
              ),
              Container(
                height: 0.5,
                color: const Color(0xFF262626),
              ),
              CardLabelTile(
                position: CardLabelTilePosition.bottom,
                label: context.tr("journey.export_journey_as_gpx"),
                onTap: () {
                  Navigator.pop(context);
                  onLabelTaped?.call(ExportType.gpx);
                },
              ),
            ]
          ],
        ),
      ),
    );
  }
}
