import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:path/path.dart' as path;
import 'package:path_provider/path_provider.dart';

Future<CommonExportResult> _generateJourneyExport(
  JourneyHeader journey,
  CommonExportFormat format,
) async {
  final temporaryDirectory = await getTemporaryDirectory();
  final date = journey.journeyDate.toSimpleDate().toString();
  final outputPath = path.join(
    temporaryDirectory.path,
    '$date-${journey.revision}.${format.extension}',
  );
  final exportType = switch (format) {
    CommonExportFormat.mldx => api.ExportType.mldx,
    CommonExportFormat.fwss => api.ExportType.fwss,
    CommonExportFormat.gpx => api.ExportType.gpx,
    CommonExportFormat.kml => api.ExportType.kml,
  };
  final result = await api.exportJourney(
    targetFilepath: outputPath,
    journeyId: journey.id,
    exportType: exportType,
  );
  return CommonExportResult.create(result, outputPath);
}

Future<void> showJourneyExportPicker(
  BuildContext context,
  JourneyHeader journey,
) async {
  final supportsVector = journey.journeyType != JourneyType.bitmap;
  await showCommonExportWithFormatPicker(
    context: context,
    title: context.tr('data.export_data.export_journey_title'),
    formats: [
      CommonExportFormat.mldx,
      CommonExportFormat.fwss,
      if (supportsVector) CommonExportFormat.kml,
      if (supportsVector) CommonExportFormat.gpx,
    ],
    exportFile: (format) => _generateJourneyExport(journey, format),
  );
}
