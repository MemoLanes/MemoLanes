import 'package:easy_localization/easy_localization.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/legacy_raw_data.dart';

class RawDataSwitch extends StatefulWidget {
  const RawDataSwitch({super.key});

  @override
  State<RawDataSwitch> createState() => _RawDataSwitchState();
}

class _RawDataSwitchState extends State<RawDataSwitch> {
  bool? enabled;
  bool _busy = true;

  @override
  void initState() {
    super.initState();
    _loadMode();
  }

  Future<void> _loadMode() async {
    try {
      final value = await api.getRawDataMode();
      if (!mounted) return;
      setState(() => enabled = value);
    } catch (error, stackTrace) {
      log.error('Failed to load raw data mode: $error', stackTrace);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _setMode(bool value) async {
    if (_busy || enabled == null) return;
    setState(() => _busy = true);
    try {
      await api.toggleRawDataMode(enable: value);
      if (!mounted) return;
      setState(() => enabled = value);
    } catch (error, stackTrace) {
      log.error('Failed to change raw data mode: $error', stackTrace);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Switch(
      value: enabled ?? false,
      onChanged: _busy || enabled == null ? null : _setMode,
    );
  }
}

class RawDataPage extends StatefulWidget {
  const RawDataPage({super.key});

  @override
  State<RawDataPage> createState() => _RawDataPage();
}

class _RawDataPage extends State<RawDataPage> {
  List<LegacyRawDataFile> items = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _loadList();
  }

  Future<void> _loadList() async {
    try {
      final list = await api.listAllLegacyRawData();
      if (!mounted) return;
      setState(() {
        items = list;
        _loading = false;
      });
    } catch (error, stackTrace) {
      log.error('Failed to load raw data files: $error', stackTrace);
      if (mounted) setState(() => _loading = false);
    }
  }

  void _showExportCard(BuildContext context, String filePath) {
    showBasicCard(
      context,
      title: context.tr("common.export"),
      builder: (dialogContext) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          AppOptionTile(
            icon: Icons.table_chart_outlined,
            title: context.tr("general.advanced_settings.raw_data_export_csv"),
            onTap: () {
              Navigator.of(dialogContext).pop();
              showCommonExport(context, filePath, deleteFile: false);
            },
          ),
          const SizedBox(height: 8),
          AppOptionTile(
            icon: Icons.route_outlined,
            title: context.tr("general.advanced_settings.raw_data_export_gpx"),
            onTap: () async {
              Navigator.of(dialogContext).pop();
              final gpxPath = await api.exportLegacyRawDataGpxFile(
                csvFilepath: filePath,
              );
              if (!context.mounted) return;
              showCommonExport(context, gpxPath, deleteFile: true);
            },
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.advanced_settings.raw_data_files"),
      ),
      body: SettingsPageFrame(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Expanded(
              child: _loading
                  ? const Center(child: CircularProgressIndicator())
                  : items.isEmpty
                  ? Center(
                      child: Text(
                        context.tr(
                          'general.advanced_settings.no_raw_data_files',
                        ),
                      ),
                    )
                  : ListView(
                      shrinkWrap: true,
                      padding: EdgeInsets.zero,
                      children: [
                        OptionCard(
                          useSafeArea: false,
                          children: items.map((item) {
                            return ListTile(
                              leading: const Icon(Icons.description),
                              title: Text(item.name),
                              onTap: () {
                                _showExportCard(context, item.path);
                              },
                              trailing: AppIconButton(
                                icon: Icons.delete_outline_rounded,
                                variant: AppButtonVariant.danger,
                                size: 38,
                                onPressed: () async {
                                  if (await showCommonDialog(
                                    context,
                                    context.tr(
                                      "journey.delete_journey_message",
                                    ),
                                    hasCancel: true,
                                    title: context.tr(
                                      "journey.delete_journey_title",
                                    ),
                                    confirmButtonText: context.tr(
                                      "common.delete",
                                    ),
                                    confirmVariant: AppButtonVariant.danger,
                                  )) {
                                    await api.deleteLegacyRawDataFile(
                                      filename: item.name,
                                    );
                                    await _loadList();
                                  }
                                },
                              ),
                            );
                          }).toList(),
                        ),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
