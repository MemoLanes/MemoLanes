import 'package:flutter/material.dart';
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
          icon: Icon(
              layerMode == LayerMode.all
                  ? Icons.layers
                  : layerMode == LayerMode.flight
                      ? Icons.flight
                      : Icons.directions_car,
              color: const Color(0xFFB4EC51)),
          tooltip: 'Layer picker not implemented',
        ),
      ),
    );
  }
}
