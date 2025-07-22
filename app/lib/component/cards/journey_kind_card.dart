import 'package:flutter/material.dart';
import 'package:memolanes/component/cards/card_label_tile.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyKindCard extends StatelessWidget {
  const JourneyKindCard({
    super.key,
    this.onLabelTaped,
  });

  final Function(JourneyKind)? onLabelTaped;

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
              label: '默认',
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call(JourneyKind.defaultKind);
              },
              top: false,
            ),
            Container(
              height: 0.5,
              color: const Color(0xFF262626),
            ),
            CardLabelTile(
              position: CardLabelTilePosition.top,
              label: '航迹',
              onTap: () {
                Navigator.pop(context);
                onLabelTaped?.call(JourneyKind.flight);
              },
            ),
          ],
        ),
      ),
    );
  }
}
