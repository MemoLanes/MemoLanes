import 'package:memolanes/body/settings/appearance_picker.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/body/settings/contact_us_section.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/body/settings/raw_data_page.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/share_handler_util.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';
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
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.recording_data'),
            children: [
              SettingsTile(
                label: context.tr('general.advanced_settings.raw_data_mode'),
                position: LabelTilePosition.top,
                bottom: false,
                labelTrailing: IconButton(
                  tooltip: context.tr(
                    'general.advanced_settings.raw_data_mode',
                  ),
                  icon: Icon(
                    Icons.help_outline_rounded,
                    color: context.appColors.mutedInkColor,
                    size: 20,
                  ),
                  onPressed: () => showCommonDialog(
                    context,
                    context.tr(
                      'general.advanced_settings.raw_data_mode_description',
                    ),
                    title: context.tr(
                      'general.advanced_settings.raw_data_mode',
                    ),
                  ),
                ),
                trailing: const RawDataSwitch(),
              ),
              SettingsTile(
                label: context.tr('journey.drop_covered_small_journey'),
                position: LabelTilePosition.bottom,
                bottom: false,
                labelTrailing: IconButton(
                  tooltip: context.tr('journey.drop_covered_small_journey'),
                  icon: Icon(
                    Icons.help_outline_rounded,
                    color: context.appColors.mutedInkColor,
                    size: 20,
                  ),
                  onPressed: () => showCommonDialog(
                    context,
                    context.tr(
                      'journey.drop_covered_small_journey_description',
                    ),
                    title: context.tr('journey.drop_covered_small_journey'),
                  ),
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
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.recording_protection'),
            children: [
              SettingsTile(
                label: context.tr('unexpected_exit_notification.setting_title'),
                position: defaultTargetPlatform == TargetPlatform.android
                    ? LabelTilePosition.top
                    : LabelTilePosition.single,
                bottom: false,
                trailing: Switch(
                  value: _notificationEnabled,
                  onChanged: (value) =>
                      _setNotification(context, value, gpsManager),
                ),
              ),
              if (defaultTargetPlatform == TargetPlatform.android)
                SettingsTile(
                  label: context.tr('recording_health.setting_title'),
                  position: LabelTilePosition.bottom,
                  bottom: false,
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
    if (value) {
      final granted = await Permission.notification.isGranted;
      if (!context.mounted) return;
      if (!granted) {
        setState(() => _notificationEnabled = false);
        await showCommonDialog(
          context,
          context.tr(
            'unexpected_exit_notification.notification_permission_denied',
          ),
        );
        await Geolocator.openAppSettings();
        return;
      }
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
    if (selected == 'system') {
      MMKVUtil.removeAppKey(MMKVKey.localePreference);
    } else {
      MMKVUtil.putString(MMKVKey.localePreference, locale.toLanguageTag());
    }
    await context.setLocale(locale);
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
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.appearance'),
            children: [
              SettingsTile(
                label: context.tr('general.appearance.title'),
                position: LabelTilePosition.top,
                trailing: LabelTileContent(
                  content: context.tr(
                    'general.appearance.mode_name.${context.watch<AppThemeController>().preference.id}',
                  ),
                  showArrow: true,
                ),
                onTap: () => showAppearancePicker(context),
              ),
              SettingsTile(
                label: context.tr('settings.language.title'),
                position: LabelTilePosition.middle,
                trailing: LabelTileContent(
                  content: _languageLabel(context),
                  showArrow: true,
                ),
                onTap: _selectLanguage,
              ),
              SettingsTile(
                label: context.tr('haptics.setting_title'),
                position: LabelTilePosition.bottom,
                bottom: false,
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
  bool _hasRawDataFiles = false;
  int _rawDataRefreshId = 0;

  @override
  void initState() {
    super.initState();
    _refreshRawDataFiles();
  }

  Future<void> _refreshRawDataFiles() async {
    final requestId = ++_rawDataRefreshId;
    try {
      final files = await api.listAllLegacyRawData();
      if (!mounted || requestId != _rawDataRefreshId) return;
      setState(() => _hasRawDataFiles = files.isNotEmpty);
    } catch (error, stackTrace) {
      log.error('Failed to list raw data files: $error', stackTrace);
      if (!mounted || requestId != _rawDataRefreshId) return;
      setState(() => _hasRawDataFiles = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.data.title'),
      ),
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.import_export'),
            children: [
              SettingsTile(
                label: context.tr('data.import_data.title'),
                position: LabelTilePosition.top,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () => _showImportDataCard(context),
              ),
              SettingsTile(
                label: context.tr('data.export_data.export_all'),
                position: LabelTilePosition.bottom,
                bottom: false,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () => _exportAll(context, gpsManager),
              ),
            ],
          ),
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.data_maintenance'),
            children: [
              if (_hasRawDataFiles)
                SettingsTile(
                  label: context.tr('general.advanced_settings.raw_data_files'),
                  position: LabelTilePosition.top,
                  trailing: const LabelTileContent(showArrow: true),
                  onTap: () async {
                    await navigatorPush(context, page: const RawDataPage());
                    if (mounted) await _refreshRawDataFiles();
                  },
                ),
              SettingsTile(
                label: context.tr('db_optimization.button'),
                position: _hasRawDataFiles
                    ? LabelTilePosition.middle
                    : LabelTilePosition.top,
                onTap: _optimizeDatabase,
              ),
              SettingsTile(
                label: context.tr('general.advanced_settings.rebuild_cache'),
                position: LabelTilePosition.bottom,
                bottom: false,
                onTap: () => showLoadingDialog(asyncTask: api.rebuildCache()),
              ),
            ],
          ),
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.delete_data'),
            children: [
              SettingsTile(
                label: context.tr('journey.delete_all'),
                labelStyle: AppTypography.itemTitle.copyWith(
                  color: context.appColors.dangerInkColor,
                ),
                prefix: Padding(
                  padding: const EdgeInsets.only(right: 12),
                  child: Icon(
                    Icons.delete_outline_rounded,
                    color: context.appColors.dangerInkColor,
                    size: 20,
                  ),
                ),
                position: LabelTilePosition.single,
                bottom: false,
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
      formatGroups: [
        CommonExportFormatGroup(
          label: context.tr('data.export_data.journey_data_group'),
          formats: const [CommonExportFormat.mldx, CommonExportFormat.fwss],
        ),
      ],
      exportFile: (selection) async {
        final format = selection.format;
        final tmpDir = await getTemporaryDirectory();
        final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss')
            .format(DateTime.now());
        final filepath =
            '${tmpDir.path}/all-journeys-$timestamp.${format.extension}';
        final result = switch (format) {
          CommonExportFormat.mldx => await api.generateFullArchive(
            targetFilepath: filepath,
            includeRawData: selection.includeRawData,
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
    if (!mounted) return;
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
      title: context.tr('journey.delete_all_title'),
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

  Future<void> _showImportDataCard(BuildContext context) async {
    final shouldSelectFile = await showBasicDialogCard<bool>(
      context,
      title: context.tr('data.import_data.title'),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
      actions: Builder(
        builder: (dialogContext) => AppButton(
          label: dialogContext.tr('data.import_data.choose_file'),
          expand: true,
          onPressed: () => Navigator.of(dialogContext).pop(true),
        ),
      ),
      builder: (dialogContext) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Text(
              dialogContext.tr('data.import_data.description'),
              style: AppTypography.supporting.copyWith(
                color: dialogContext.appColors.mutedInkColor,
              ),
            ),
          ),
          _ImportFormatInfoRow(
            icon: Icons.archive_outlined,
            title: dialogContext.tr('data.import_data.mldx_title'),
            subtitle: dialogContext.tr('data.import_data.mldx_desc'),
          ),
          const SizedBox(height: 8),
          _ImportFormatInfoRow(
            icon: Icons.route_outlined,
            title: dialogContext.tr('data.import_data.vector_title'),
            subtitle: dialogContext.tr('data.import_data.vector_desc'),
          ),
          const SizedBox(height: 8),
          _ImportFormatInfoRow(
            icon: Icons.public_outlined,
            title: dialogContext.tr('data.import_data.fog_of_world_title'),
            subtitle: dialogContext.tr('data.import_data.fog_of_world_desc'),
          ),
        ],
      ),
    );

    if (shouldSelectFile != true || !context.mounted) return;
    final file = await FilePicker.pickFile(type: FileType.any);
    if (file == null || !context.mounted) return;
    final isMldx = file.name.toLowerCase().endsWith('.mldx');
    final importType = ShareHandlerUtil.resolveImportType(file.name);
    if (!isMldx && importType == null) {
      await showCommonDialog(
        context,
        context.tr('data.import_data.unsupported_format'),
      );
      return;
    }
    final path = file.path;
    if (path == null) {
      await showCommonDialog(
        context,
        context.tr('data.import_data.unreadable_file'),
      );
      return;
    }
    if (isMldx) {
      await importMldx(context, path);
      return;
    }

    final selectedType = importType!;

    if (selectedType == ImportType.vector) {
      await showCommonDialog(
        context,
        context.tr('import.vector.description_md'),
        markdown: true,
      );
    } else {
      await showCommonDialog(
        context,
        context.tr('import.import_fow_data.description_md'),
        markdown: true,
      );
      if (!context.mounted) return;
      if (await api.containsBitmapJourney()) {
        if (!context.mounted) return;
        await showCommonDialog(
          context,
          context.tr(
            'import.import_fow_data.warning_for_import_multiple_data_md',
          ),
          markdown: true,
        );
      }
    }
    if (!context.mounted) return;
    navigatorPush(
      context,
      page: ImportDataPage(path: path, importType: selectedType),
    );
  }
}

class _ImportFormatInfoRow extends StatelessWidget {
  const _ImportFormatInfoRow({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: const BoxConstraints(minHeight: 58),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: context.appColors.surfaceColor.withValues(alpha: 0.76),
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: context.appColors.lineColor),
      ),
      child: Row(
        children: [
          Container(
            width: 36,
            height: 36,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: context.appColors.softGreen,
              borderRadius: BorderRadius.circular(11),
            ),
            child: Icon(icon, color: context.appColors.deepGreen, size: 20),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: AppTypography.itemTitle.copyWith(
                    color: context.appColors.inkColor,
                  ),
                ),
                const SizedBox(height: 3),
                Text(
                  subtitle,
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                  style: AppTypography.caption.copyWith(
                    color: context.appColors.mutedInkColor,
                  ),
                ),
              ],
            ),
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
  String _commitHash = '';
  bool _showCommitHash = false;
  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then((info) {
      if (mounted) {
        setState(() {
          _version = '${info.version} (${info.buildNumber})';
          _commitHash = api.shortCommitHash();
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final updateNotifier = context.watch<UpdateNotifier>();
    final updateUrl = updateNotifier.updateUrl;
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.about.title'),
      ),
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.about'),
            children: [
              SettingsTile(
                label: context.tr('general.version.title'),
                position: LabelTilePosition.top,
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (updateNotifier.hasUpdateNotification()) ...[
                      badges.Badge(
                        badgeStyle: badges.BadgeStyle(
                          shape: badges.BadgeShape.square,
                          borderRadius: BorderRadius.circular(5),
                          padding: const EdgeInsets.all(2),
                          badgeColor: context.appColors.deepGreen,
                        ),
                        badgeContent: Text(
                          'NEW',
                          style: AppTypography.badge.copyWith(
                            color: context.appColors.inverseInkColor,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                    ],
                    GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () =>
                          setState(() => _showCommitHash = !_showCommitHash),
                      child: LabelTileContent(
                        content: _showCommitHash ? _commitHash : _version,
                      ),
                    ),
                  ],
                ),
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
                    Fluttertoast.showToast(
                      msg: context.tr(
                        'general.version.currently_the_latest_version',
                      ),
                    );
                  }
                },
              ),
              SettingsTile(
                label: context.tr('privacy.name'),
                position: LabelTilePosition.bottom,
                bottom: false,
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
