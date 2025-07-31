import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/component/cards/card_label_tile.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';

class ImportDataCard extends StatelessWidget {
  const ImportDataCard({
    super.key,
    this.onLabelTaped,
  });

  final Function(String)? onLabelTaped;

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
              position: CardLabelTilePosition.top,
              label: context.tr("journey.import_mldx_data"),
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call('MLDX');
              },
              top: false,
            ),
            Container(
              height: 0.5,
              color: const Color(0xFF262626),
            ),
            CardLabelTile(
              position: CardLabelTilePosition.middle,
              label: context.tr("journey.import_kml_gpx_data"),
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call('KML/GPX');
              },
            ),
            Container(
              height: 0.5,
              color: const Color(0xFF262626),
            ),
            CardLabelTile(
              position: CardLabelTilePosition.bottom,
              label: context.tr("journey.import_fog_of_world_data"),
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call('FOG_OF_WORLD');
              },
            ),
          ],
        ),
      ),
    );
  }
}
