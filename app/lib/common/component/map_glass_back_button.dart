import 'package:flutter/material.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Back button shared by cards and controls floating directly over the map.
class MapGlassBackButton extends StatelessWidget {
  const MapGlassBackButton({super.key, required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return LiquidGlassSurface(
      circular: true,
      backgroundAlpha: 0.62,
      blurSigma: 28,
      reflectionAlpha: 0.1,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onPressed,
          customBorder: const CircleBorder(),
          child: SizedBox.square(
            dimension: 42,
            child: Icon(
              Icons.arrow_back_ios_new_rounded,
              size: 18,
              color: StyleConstants.deepGreen,
              semanticLabel: MaterialLocalizations.of(context)
                  .backButtonTooltip,
            ),
          ),
        ),
      ),
    );
  }
}
