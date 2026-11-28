import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/service/location/location_service_factory.dart';
import 'package:memolanes/common/utils.dart';

Widget locationBackendSettingsTile(
  BuildContext context,
  GpsManager gpsManager,
) {
  final canChooseBackend = supportedLocationBackends.length > 1;
  return SettingsTile(
    label: context.tr('location_service.location_backend.title'),
    position: LabelTilePosition.middle,
    trailing: LabelTileContent(
      content: gpsManager.locationBackend.displayName(context),
      showArrow: canChooseBackend,
    ),
    onTap: canChooseBackend ? () => _chooseBackend(context, gpsManager) : null,
  );
}

Future<void> _chooseBackend(BuildContext context, GpsManager gpsManager) async {
  final backend = await showAppDialog<LocationBackend>(
    context,
    builder: (dialogContext) => AppDialogContent(
      title: dialogContext.tr('location_service.location_backend.title'),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        spacing: 8,
        children: [
          for (final backend in supportedLocationBackends)
            AppOptionTile(
              title: backend.displayName(dialogContext),
              selected: gpsManager.locationBackend == backend,
              trailing: AppOptionTileTrailing.selection,
              onTap: () => popCurrentRoute(dialogContext, backend),
            ),
        ],
      ),
    ),
  );
  if (!context.mounted || backend == null) return;

  String reason;
  try {
    if (await gpsManager.setLocationBackend(backend)) return;
    reason = 'journey.stop_ongoing_journey';
  } catch (error, stackTrace) {
    log.error(
      '[Location service] Changing location provider failed: $error',
      stackTrace,
    );
    reason = 'location_service.location_backend.switch_failed';
  }
  if (context.mounted) await showCommonDialog(context, context.tr(reason));
}
