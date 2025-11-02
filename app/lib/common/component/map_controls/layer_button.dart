import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class LayerButton extends StatelessWidget {
  final Set<api.LayerKind> activeLayers;
  final ValueChanged<Set<api.LayerKind>> onLayersChanged;

  const LayerButton({
    super.key,
    required this.activeLayers,
    required this.onLayersChanged,
  });

  @override
  Widget build(BuildContext context) {
    return CustomPopup(
      position: PopupPosition.left,
      horizontalOffset: -16,
      contentRadius: 24,
      barrierColor: Colors.transparent,
      content: PointerInterceptor(
          child: LayerPopupContent(
        initialActiveLayers: activeLayers,
        onChanged: onLayersChanged,
      )),
      child: PointerInterceptor(
          child: Container(
        width: 48,
        height: 48,
        decoration: const BoxDecoration(
          color: Colors.black,
          shape: BoxShape.circle,
        ),
        child: Center(
          child: Icon(
            Icons.layers,
            color: StyleConstants.defaultColor,
            size: 20,
          ),
        ),
      )),
    );
  }
}

class LayerPopupContent extends StatefulWidget {
  final Set<api.LayerKind> initialActiveLayers;
  final ValueChanged<Set<api.LayerKind>> onChanged;

  const LayerPopupContent({
    super.key,
    required this.initialActiveLayers,
    required this.onChanged,
  });

  @override
  State<LayerPopupContent> createState() => _LayerPopupContentState();
}

class _LayerPopupContentState extends State<LayerPopupContent> {
  late Set<api.LayerKind> _activeLayers;

  @override
  void initState() {
    super.initState();
    _activeLayers = Set.from(widget.initialActiveLayers);
  }

  void _toggle(api.LayerKind kind) {
    setState(() {
      if (_activeLayers.contains(kind)) {
        _activeLayers.remove(kind);
      } else {
        _activeLayers.add(kind);
      }
      widget.onChanged(_activeLayers);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // TODO wait rust chang layer kind enum
        _buildItem(api.LayerKind.defaultKind,
            context.tr("journey_kind.current"), FontAwesomeIcons.locationDot),
        _buildItem(api.LayerKind.flight, context.tr("journey_kind.default"),
            FontAwesomeIcons.shoePrints),
        _buildItem(api.LayerKind.all, context.tr("journey_kind.flight"),
            FontAwesomeIcons.planeUp),
      ],
    );
  }

  Widget _buildItem(api.LayerKind kind, String text, IconData icon) {
    final isActive = _activeLayers.contains(kind);

    return InkWell(
      onTap: () => _toggle(kind),
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 8),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon,
                color: isActive ? StyleConstants.defaultColor : Colors.white70,
                size: 16),
            const SizedBox(width: 8),
            Text(
              text,
              style: TextStyle(
                color: isActive ? StyleConstants.defaultColor : Colors.white70,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
