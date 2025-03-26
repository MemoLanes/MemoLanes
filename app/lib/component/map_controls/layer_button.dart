import 'package:flutter/material.dart';

import '../../src/rust/journey_header.dart';

class LayerButton extends StatelessWidget {
  final JourneyKind layerMode;
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
          icon: Icon(
            Icons.layers,
            color: layerMode == JourneyKind.defaultKind
                ? const Color(0xFFB4EC51)
                : Colors.white38,
          ),
          tooltip: 'Layer picker not implemented',
        ),
      ),
    );
  }
}
