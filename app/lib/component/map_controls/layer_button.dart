import 'package:flutter/material.dart';

class LayerButton extends StatelessWidget {
  final VoidCallback onPressed;

  const LayerButton({
    super.key,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.all(8),
      width: 48,
      height: 48,
      decoration: const BoxDecoration(
        color: Colors.black38, // TODO: undisable
        shape: BoxShape.circle,
      ),
      child: const Material(
        color: Colors.transparent,
        child: IconButton(
          onPressed: null, // TODO: undisable
          icon: Icon(
            Icons.layers,
            color: Colors.white38, // TODO: undisable
          ),
          tooltip: 'Layer picker not implemented',
        ),
      ),
    );
  }
}
