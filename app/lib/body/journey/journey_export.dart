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
  CommonExportSelection selection,
) async {
  final format = selection.format;
  final temporaryDirectory = await getTemporaryDirectory();
  final date = journey.journeyDate.toSimpleDate().toString();
  final outputPath = path.join(
    temporaryDirectory.path,
    '$date-${journey.revision}${format.isRawData ? '-raw' : ''}.${format.extension}',
  );

  if (format.isRawData) {
    final exportType = switch (format) {
      CommonExportFormat.rawDataCsv => api.RawDataExportType.csv,
      CommonExportFormat.rawDataGpx => api.RawDataExportType.gpx,
      CommonExportFormat.rawDataKml => api.RawDataExportType.kml,
      _ => throw UnsupportedError(
        'Unsupported raw-data export format: $format',
      ),
    };
    final result = await api.exportJourneyRawData(
      targetFilepath: outputPath,
      journeyId: journey.id,
      exportType: exportType,
    );
    return CommonExportResult.create(result, outputPath);
  }

  final exportType = switch (format) {
    CommonExportFormat.mldx => api.ExportType.mldx,
    CommonExportFormat.fwss => api.ExportType.fwss,
    CommonExportFormat.gpx => api.ExportType.gpx,
    CommonExportFormat.kml => api.ExportType.kml,
    CommonExportFormat.rawDataCsv ||
    CommonExportFormat.rawDataGpx ||
    CommonExportFormat.rawDataKml => throw UnsupportedError(
      'Unsupported journey export format: $format',
    ),
  };
  final result = await api.exportJourney(
    targetFilepath: outputPath,
    journeyId: journey.id,
    exportType: exportType,
    includeRawData: selection.includeRawData,
  );
  return CommonExportResult.create(result, outputPath);
}

Future<void> showJourneyExportPicker(
  BuildContext context,
  JourneyHeader journey, {
  required bool hasRawData,
}) async {
  final supportsVector = journey.journeyType != JourneyType.bitmap;
  await showCommonExportWithFormatPicker(
    context: context,
    title: context.tr('data.export_data.export_journey_title'),
    canIncludeRawData: hasRawData,
    formatGroups: [
      CommonExportFormatGroup(
        label: context.tr('data.export_data.journey_data_group'),
        formats: [
          CommonExportFormat.mldx,
          CommonExportFormat.fwss,
          if (supportsVector) CommonExportFormat.kml,
          if (supportsVector) CommonExportFormat.gpx,
        ],
      ),
      if (hasRawData)
        CommonExportFormatGroup(
          label: context.tr('data.export_data.raw_data_group'),
          formats: const [
            CommonExportFormat.rawDataCsv,
            CommonExportFormat.rawDataGpx,
            CommonExportFormat.rawDataKml,
          ],
          lossyFormatWarning: context.tr(
            'journey.raw_data_lossy_format_warning',
          ),
        ),
    ],
    exportFile: (selection) => _generateJourneyExport(journey, selection),
  );
}
