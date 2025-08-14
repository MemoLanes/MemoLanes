import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart';

class LayerButton extends StatelessWidget {
  final LayerKind layerKind;
  final VoidCallback onPressed;

  const LayerButton({
    super.key,
    required this.onPressed,
    required this.layerKind,
  });

  @override
  Widget build(BuildContext context) {
    final defaultColor = Color(0xFFB4EC51);
    return Container(
      margin: const EdgeInsets.only(top: 8, bottom: 8),
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        color: Colors.black,
        shape: BoxShape.circle,
      ),
      child: Material(
        color: Colors.transparent,
        child: IconButton(
          onPressed: onPressed,
          icon: switch (layerKind) {
            LayerKind.defaultKind => FaIcon(
                FontAwesomeIcons.shoePrints,
                color: defaultColor,
                size: 18,
              ),
            LayerKind.flight => FaIcon(
                FontAwesomeIcons.planeUp,
                color: defaultColor,
                size: 18,
              ),
            LayerKind.all => Icon(
                Icons.layers,
                color: defaultColor,
              )
          },
          tooltip: 'Layer picker not implemented',
        ),
      ),
    );
  }
}
