import 'package:flutter/material.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';

import 'card_label_tile.dart';

class ExportDataCard extends StatelessWidget {
  const ExportDataCard({
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
              label: 'MLDX',
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
              position: CardLabelTilePosition.top,
              label: 'KML',
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call('KML');
              },
            ),
            Container(
              height: 0.5,
              color: const Color(0xFF262626),
            ),
            CardLabelTile(
              position: CardLabelTilePosition.top,
              label: 'GPX',
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call('GPX');
              },
            ),
          ],
        ),
      ),
    );
  }
}
