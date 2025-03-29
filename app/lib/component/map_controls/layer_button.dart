import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart';

class LayerButton extends StatelessWidget {
  final LayerMode layerMode;
  final VoidCallback onPressed;

  const LayerButton({
    super.key,
    required this.onPressed,
    required this.layerMode,
  });

  @override
  Widget build(BuildContext context) {
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
          icon: FaIcon(
            layerMode == LayerMode.all
                ? FontAwesomeIcons.layerGroup
                : layerMode == LayerMode.flight
                    ? FontAwesomeIcons.cloud
                    : FontAwesomeIcons.shoePrints,
            color: const Color(0xFFB4EC51),
            size: 18,
          ),
          tooltip: 'Layer picker not implemented',
        ),
      ),
    );
  }
}
