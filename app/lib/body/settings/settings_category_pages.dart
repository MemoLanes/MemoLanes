import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/body/settings/contact_us_section.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/body/settings/raw_data_page.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:url_launcher/url_launcher_string.dart';

class JourneyRecordingSettingsPage extends StatefulWidget {
  const JourneyRecordingSettingsPage({super.key});

  @override
  State<JourneyRecordingSettingsPage> createState() =>
      _JourneyRecordingSettingsPageState();
}

class _JourneyRecordingSettingsPageState
    extends State<JourneyRecordingSettingsPage> {
  late bool _notificationEnabled;
  late bool _dropCoveredSmallJourneyEnabled;

  @override
  void initState() {
    super.initState();
    _notificationEnabled = MMKVUtil.getBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      defaultValue: true,
    );
    _dropCoveredSmallJourneyEnabled = MMKVUtil.getBool(
      MMKVKey.dropCoveredSmallJourneyEnabled,
      defaultValue: true,
    );
  }

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.journey_recording.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          SettingsSection(
            title: context.tr('settings.groups.journey'),
            children: [
              LabelTile(
                label: context.tr('journey.drop_covered_small_journey'),
                position: LabelTilePosition.single,
                infoLabelOnTap: () => showCommonDialog(
                  context,
                  context.tr('journey.drop_covered_small_journey_description'),
                  title: context.tr('journey.drop_covered_small_journey'),
                ),
                trailing: Switch(
                  value: _dropCoveredSmallJourneyEnabled,
                  onChanged:
                      gpsManager.recordingStatus == GpsRecordingStatus.none
                      ? (value) {
                          MMKVUtil.putBool(
                            MMKVKey.dropCoveredSmallJourneyEnabled,
                            value,
                          );
                          setState(
                            () => _dropCoveredSmallJourneyEnabled = value,
                          );
                        }
                      : null,
                ),
              ),
            ],
          ),
          SettingsSection(
            title: context.tr('settings.groups.recording_protection'),
            children: [
              LabelTile(
                label: context.tr('unexpected_exit_notification.setting_title'),
                position: defaultTargetPlatform == TargetPlatform.android
                    ? LabelTilePosition.top
                    : LabelTilePosition.single,
                trailing: Switch(
                  value: _notificationEnabled,
                  onChanged: (value) =>
                      _setNotification(context, value, gpsManager),
                ),
              ),
              if (defaultTargetPlatform == TargetPlatform.android)
                LabelTile(
                  label: context.tr('recording_health.setting_title'),
                  position: LabelTilePosition.bottom,
                  trailing: ListenableBuilder(
                    listenable: RecordingHealthService.instance,
                    builder: (context, _) => Switch(
                      value: RecordingHealthService
                          .instance
                          .isHeartbeatDetectionEnabled,
                      onChanged: (value) => RecordingHealthService.instance
                          .setHeartbeatDetectionEnabled(
                            value,
                            recordingStatus: gpsManager.recordingStatus,
                          ),
                    ),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _setNotification(
    BuildContext context,
    bool value,
    GpsManager gpsManager,
  ) async {
    if (value && !await Permission.notification.isGranted) {
      if (!context.mounted) return;
      setState(() => _notificationEnabled = false);
      if (!context.mounted) return;
      await showCommonDialog(
        context,
        context.tr(
          'unexpected_exit_notification.notification_permission_denied',
        ),
      );
      await Geolocator.openAppSettings();
      return;
    }
    MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, value);
    setState(() => _notificationEnabled = value);
    if (gpsManager.recordingStatus == GpsRecordingStatus.recording &&
        context.mounted) {
      await showCommonDialog(
        context,
        context.tr('unexpected_exit_notification.change_affect_next_time'),
      );
    }
  }
}

class AppearanceSettingsPage extends StatefulWidget {
  const AppearanceSettingsPage({super.key});

  @override
  State<AppearanceSettingsPage> createState() => _AppearanceSettingsPageState();
}

class _AppearanceSettingsPageState extends State<AppearanceSettingsPage> {
  String _languageLabel(BuildContext context) {
    final localePreference = MMKVUtil.getStringOpt(MMKVKey.localePreference);
    if (localePreference == null) return context.tr('settings.language.system');
    return localePreference.startsWith('zh')
        ? context.tr('settings.language.chinese')
        : context.tr('settings.language.english');
  }

  Future<void> _selectLanguage() async {
    final selected = await showBasicCard<String>(
      context,
      title: context.tr('settings.language.title'),
      builder: (dialogContext) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          _languageOption(
            dialogContext,
            'system',
            context.tr('settings.language.system'),
          ),
          const SizedBox(height: 8),
          _languageOption(
            dialogContext,
            'zh-CN',
            context.tr('settings.language.chinese'),
          ),
          const SizedBox(height: 8),
          _languageOption(
            dialogContext,
            'en-US',
            context.tr('settings.language.english'),
          ),
        ],
      ),
    );
    if (!mounted || selected == null) return;
    final locale = switch (selected) {
      'zh-CN' => const Locale('zh', 'CN'),
      'en-US' => const Locale('en', 'US'),
      _ => _systemLocale(),
    };
    await context.setLocale(locale);
    if (selected == 'system') {
      MMKVUtil.removeAppKey(MMKVKey.localePreference);
    } else {
      MMKVUtil.putString(MMKVKey.localePreference, locale.toLanguageTag());
    }
    if (mounted) setState(() {});
  }

  Locale _systemLocale() {
    return WidgetsBinding.instance.platformDispatcher.locale.languageCode ==
            'zh'
        ? const Locale('zh', 'CN')
        : const Locale('en', 'US');
  }

  Widget _languageOption(BuildContext context, String value, String title) {
    final saved = MMKVUtil.getStringOpt(MMKVKey.localePreference);
    final selected = value == 'system' ? saved == null : saved == value;
    return AppOptionTile(
      title: title,
      selected: selected,
      trailing: AppOptionTileTrailing.selection,
      onTap: () => Navigator.of(context).pop(value),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.appearance.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          SettingsSection(
            title: context.tr('settings.groups.appearance'),
            children: [
              LabelTile(
                label: context.tr('settings.language.title'),
                position: LabelTilePosition.top,
                trailing: LabelTileContent(
                  content: _languageLabel(context),
                  showArrow: true,
                ),
                onTap: _selectLanguage,
              ),
              LabelTile(
                label: context.tr('haptics.setting_title'),
                position: LabelTilePosition.bottom,
                trailing: Switch(
                  value: AppHaptics.isUserHapticsEnabled,
                  onChanged: (value) {
                    AppHaptics.setUserHapticsEnabled(value);
                    if (value) AppHaptics.selection();
                    setState(() {});
                  },
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class DataManagementSettingsPage extends StatefulWidget {
  const DataManagementSettingsPage({super.key});

  @override
  State<DataManagementSettingsPage> createState() =>
      _DataManagementSettingsPageState();
}

class _DataManagementSettingsPageState
    extends State<DataManagementSettingsPage> {
  Future<void> _selectImportFile(
    BuildContext context,
    ImportType importType,
  ) async {
    final result = await FilePicker.pickFile(type: FileType.any);
    if (result?.path != null && context.mounted) {
      navigatorPush(
        context,
        page: ImportDataPage(path: result!.path!, importType: importType),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.data.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          SettingsSection(
            title: context.tr('settings.groups.data_exchange'),
            children: [
              LabelTile(
                label: context.tr('data.import_data.title'),
                position: LabelTilePosition.top,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () => _showImportDataCard(context),
              ),
              LabelTile(
                label: context.tr('data.export_data.export_all'),
                position: LabelTilePosition.bottom,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () => _exportAll(context, gpsManager),
              ),
            ],
          ),
          SettingsSection(
            title: context.tr('settings.groups.data_maintenance'),
            children: [
              LabelTile(
                label: context.tr('general.advanced_settings.raw_data_mode'),
                position: LabelTilePosition.top,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () => navigatorPush(context, page: const RawDataPage()),
              ),
              LabelTile(
                label: context.tr('db_optimization.button'),
                position: LabelTilePosition.middle,
                onTap: _optimizeDatabase,
              ),
              LabelTile(
                label: context.tr('general.advanced_settings.rebuild_cache'),
                position: LabelTilePosition.bottom,
                onTap: () => showLoadingDialog(asyncTask: api.rebuildCache()),
              ),
            ],
          ),
          SettingsSection(
            title: context.tr('settings.groups.danger_zone'),
            children: [
              LabelTile(
                label: context.tr('journey.delete_all'),
                position: LabelTilePosition.single,
                onTap: () => _deleteAll(context, gpsManager),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _exportAll(BuildContext context, GpsManager gpsManager) async {
    if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
      await showCommonDialog(
        context,
        context.tr('journey.stop_ongoing_journey'),
      );
      return;
    }
    if (!await api.hasJourneys()) {
      if (context.mounted) {
        await showCommonDialog(
          context,
          context.tr('data.export_data.error.no_journeys_to_export'),
        );
      }
      return;
    }
    if (!context.mounted) return;
    await showCommonExportWithFormatPicker(
      context: context,
      title: context.tr('data.export_data.export_all_title'),
      formats: const [CommonExportFormat.mldx, CommonExportFormat.fwss],
      exportFile: (format) async {
        final tmpDir = await getTemporaryDirectory();
        final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss')
            .format(DateTime.now());
        final filepath =
            '${tmpDir.path}/all-journeys-$timestamp.${format.extension}';
        final result = switch (format) {
          CommonExportFormat.mldx => await api.generateFullArchive(
            targetFilepath: filepath,
          ),
          CommonExportFormat.fwss => await api.exportAllJourneysAsFwss(
            targetFilepath: filepath,
          ),
          _ => throw UnsupportedError('Unsupported export format: $format'),
        };
        return CommonExportResult.create(result, filepath);
      },
    );
  }

  Future<void> _optimizeDatabase() async {
    if (!await api.mainDbRequireOptimization()) {
      if (mounted) {
        await showCommonDialog(
          context,
          context.tr('db_optimization.already_optimized'),
        );
      }
      return;
    }
    if (!mounted ||
        !await showCommonDialog(
          context,
          context.tr('db_optimization.confirm'),
          hasCancel: true,
        )) {
      return;
    }
    await showLoadingDialog(asyncTask: api.optimizeMainDb());
    if (mounted) {
      await showCommonDialog(context, context.tr('db_optimization.finish'));
    }
  }

  Future<void> _deleteAll(BuildContext context, GpsManager gpsManager) async {
    if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
      await showCommonDialog(
        context,
        context.tr('journey.stop_ongoing_journey'),
      );
      return;
    }
    if (!await showCommonDialog(
      context,
      context.tr('journey.delete_all_journey_message'),
      hasCancel: true,
      title: context.tr('journey.delete_journey_title'),
      confirmButtonText: context.tr('common.delete'),
      confirmVariant: AppButtonVariant.danger,
    )) {
      return;
    }
    try {
      await api.deleteAllJourneys();
      if (context.mounted) {
        await showCommonDialog(
          context,
          context.tr('journey.delete_all_success'),
        );
      }
    } catch (e) {
      if (context.mounted) {
        await showCommonDialog(context, e.toString());
      }
    }
  }

  void _showImportDataCard(BuildContext context) {
    showBasicCard(
      context,
      builder: (_) => OptionCard(
        useSafeArea: false,
        embedded: true,
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr('journey.import_mldx_data'),
            top: false,
            onTap: () async {
              final result = await FilePicker.pickFile(type: FileType.any);
              if (result?.path != null && context.mounted) {
                await importMldx(context, result!.path!);
              }
            },
          ),
          CardLabelTile(
            position: CardLabelTilePosition.middle,
            label: context.tr('import.vector.title'),
            onTap: () async {
              await showCommonDialog(
                context,
                context.tr('import.vector.description_md'),
                markdown: true,
              );
              if (context.mounted) {
                await _selectImportFile(context, ImportType.vector);
              }
            },
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr('journey.import_fog_of_world_data'),
            onTap: () async {
              await showCommonDialog(
                context,
                context.tr('import.import_fow_data.description_md'),
                markdown: true,
              );
              if (await api.containsBitmapJourney() && context.mounted) {
                await showCommonDialog(
                  context,
                  context.tr(
                    'import.import_fow_data.warning_for_import_multiple_data_md',
                  ),
                  markdown: true,
                );
              }
              if (context.mounted) {
                await _selectImportFile(context, ImportType.fow);
              }
            },
          ),
        ],
      ),
    );
  }
}

class AboutSettingsPage extends StatefulWidget {
  const AboutSettingsPage({super.key});
  @override
  State<AboutSettingsPage> createState() => _AboutSettingsPageState();
}

class _AboutSettingsPageState extends State<AboutSettingsPage> {
  String _version = '';
  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then((info) {
      if (mounted) {
        setState(
          () => _version =
              '${info.version} (${info.buildNumber}) [${api.shortCommitHash()}]',
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final updateUrl = context.watch<UpdateNotifier>().updateUrl;
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.about.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          SettingsSection(
            title: context.tr('settings.about'),
            children: [
              LabelTile(
                label: context.tr('general.version.title'),
                position: LabelTilePosition.top,
                trailing: updateUrl != null
                    ? badges.Badge(
                        badgeStyle: badges.BadgeStyle(
                          shape: badges.BadgeShape.square,
                          borderRadius: BorderRadius.circular(5),
                          padding: const EdgeInsets.all(2),
                          badgeGradient: const badges.BadgeGradient.linear(
                            colors: [
                              Color(0xFFB7CC1F),
                              Color(0xFFB6E13D),
                              Color(0xFFB7CC1F),
                            ],
                          ),
                        ),
                        badgeContent: const Text(
                          'NEW',
                          style: TextStyle(color: Colors.white, fontSize: 8),
                        ),
                        child: LabelTileContent(content: _version),
                      )
                    : LabelTileContent(content: _version),
                onTap: () async {
                  if (updateUrl != null) {
                    final url = Uri.parse(updateUrl);
                    if (await canLaunchUrl(url)) {
                      await launchUrl(
                        url,
                        mode: LaunchMode.externalApplication,
                      );
                    }
                  } else if (context.mounted) {
                    await showCommonDialog(
                      context,
                      context.tr(
                        'general.version.currently_the_latest_version',
                      ),
                    );
                  }
                },
              ),
              LabelTile(
                label: context.tr('privacy.name'),
                position: LabelTilePosition.bottom,
                trailing: const LabelTileContent(rightIcon: Icons.open_in_new),
                onTap: () => launchUrlString(
                  context.tr('privacy.url'),
                  mode: LaunchMode.externalApplication,
                ),
              ),
            ],
          ),
          const ContactUsSection(),
        ],
      ),
    );
  }
}
