import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart'
    show formatJourneyInfoTime;
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/src/rust/journey_header.dart';

/// Export typography, without the interactive detail card's surface or controls.
class ScreenshotCaption extends StatelessWidget {
  const ScreenshotCaption({super.key, this.journey, this.timeRange})
    : assert((journey != null) != (timeRange != null));

  final JourneyHeader? journey;
  final DateTimeRange? timeRange;

  @override
  Widget build(BuildContext context) {
    final journey = this.journey;
    final range = timeRange;
    final format = DateFormat('yyyy.MM.dd');
    final title = journey != null
        ? format.format(journey.journeyDate)
        : range!.start == range.end
        ? format.format(range.start)
        : '${format.format(range.start)} — ${format.format(range.end)}';
    final note = journey?.note?.trim();
    return DefaultTextStyle(
      style: const TextStyle(
        fontSize: 13,
        height: 1.5,
        color: Colors.white,
        shadows: [Shadow(color: Color(0x80000000), blurRadius: 6)],
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            journey != null
                ? journeyKindLabel(context, journey.journeyKind)
                : context.tr('screenshot.selected_time'),
            style: const TextStyle(
              fontSize: 12,
              color: Color(0xffc9e8cd),
              letterSpacing: 1,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            title,
            style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w600),
          ),
          if (journey != null &&
              (journey.start != null || journey.end != null)) ...[
            const SizedBox(height: 6),
            Text(
              '${formatJourneyInfoTime(journey.start)} — ${formatJourneyInfoTime(journey.end)}',
            ),
          ],
          if (note != null && note.isNotEmpty) ...[
            const SizedBox(height: 6),
            Text(note, maxLines: 2, overflow: TextOverflow.ellipsis),
          ],
        ],
      ),
    );
  }
}
