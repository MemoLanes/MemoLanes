import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/src/rust/journey_header.dart';

/// Keeps journey-layer iconography consistent across map filters, lists, and
/// editing controls.
FaIconData journeyKindIconData(JourneyKind kind) => switch (kind) {
      JourneyKind.defaultKind => FontAwesomeIcons.shoePrints,
      JourneyKind.flight => FontAwesomeIcons.planeUp,
    };

/// Renders a journey-layer icon with Font Awesome's own widget. FaIconData is
/// intentionally not passed to Flutter's [Icon], because the two data types
/// are unrelated in font_awesome_flutter 11.
class JourneyKindIcon extends StatelessWidget {
  const JourneyKindIcon({
    super.key,
    required this.kind,
    this.size,
    this.color,
  });

  final JourneyKind kind;
  final double? size;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return FaIcon(
      journeyKindIconData(kind),
      size: size,
      color: color,
    );
  }
}
