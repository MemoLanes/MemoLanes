import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/map_controls/map_floating_control.dart';

/// Back button shared by cards and controls floating directly over the map.
class MapGlassBackButton extends StatelessWidget {
  const MapGlassBackButton({super.key, required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    // Inline headers reserve 42 pt; the Duo rail supplies its own slot size.
    return SizedBox.square(
      dimension: 42,
      child: MapFloatingControl.button(
        icon: Icons.arrow_back_ios_new_rounded,
        onPressed: onPressed,
        tooltip: MaterialLocalizations.of(context).backButtonTooltip,
      ),
    );
  }
}
