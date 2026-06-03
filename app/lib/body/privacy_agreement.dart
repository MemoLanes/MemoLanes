import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:url_launcher/url_launcher_string.dart';

const int _latestVersion = 1;
const double _setupTileMinHeight = 68.0;

enum RegionPreference {
  mainlandChina('mainland_china'),
  international('international'),
  unitedStates('united_states');

  const RegionPreference(this.storageValue);

  final String storageValue;

  static RegionPreference? fromStorageValue(String? value) {
    for (final preference in RegionPreference.values) {
      if (preference.storageValue == value) return preference;
    }
    return null;
  }
}

Future<void> _showPrivacyAndRegionSheet(
  BuildContext context, {
  required bool privacyAlreadyAccepted,
}) async {
  if (!privacyAlreadyAccepted) {
    // NOTE: we also use the same mechanism to show public beta testing notice.
    await showCommonDialog(context, context.tr("beta_testing_notice.content_md"),
        title: context.tr("beta_testing_notice.title"), markdown: true);
  }

  // A little weird, but shouldn't happen.
  if (!context.mounted) return;

  final result = await showModalBottomSheet<bool>(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    isDismissible: false,
    enableDrag: false,
    builder: (context) {
      return _PrivacyAndRegionSheetContent(
        initialPrivacyAccepted: privacyAlreadyAccepted,
      );
    },
  );

  if (result == true) {
    MMKVUtil.putInt(MMKVKey.privacyAgreementAccepted, _latestVersion);
  } else {
    exit(1);
  }
}

/// If privacy / welcome UI must be shown, returns its [Future]; otherwise a
/// completed future. Callers should await this so later steps (e.g. the
/// permission sheet) run only after those dialogs are dismissed.
Future<void> showPrivacyAgreementIfNeeded(BuildContext context) {
  var acceptedVersion =
      MMKVUtil.getInt(MMKVKey.privacyAgreementAccepted, defaultValue: 0);
  final privacyAlreadyAccepted = acceptedVersion >= _latestVersion;
  final regionPreference = MMKVUtil.getStringOpt(MMKVKey.regionPreference);
  if (!privacyAlreadyAccepted || regionPreference == null) {
    return _showPrivacyAndRegionSheet(
      context,
      privacyAlreadyAccepted: privacyAlreadyAccepted,
    );
  }
  return Future.value();
}

class _PrivacyAndRegionSheetContent extends StatefulWidget {
  const _PrivacyAndRegionSheetContent({
    required this.initialPrivacyAccepted,
  });

  final bool initialPrivacyAccepted;

  @override
  State<_PrivacyAndRegionSheetContent> createState() =>
      _PrivacyAndRegionSheetContentState();
}

class _PrivacyAndRegionSheetContentState
    extends State<_PrivacyAndRegionSheetContent> {
  RegionPreference? _selectedRegion;
  late bool _privacyAccepted;

  bool get _canContinue => _selectedRegion != null && _privacyAccepted;

  @override
  void initState() {
    super.initState();
    _privacyAccepted = widget.initialPrivacyAccepted;
    _selectedRegion = _initialRegionPreference();
  }

  RegionPreference _initialRegionPreference() {
    final storedPreference = RegionPreference.fromStorageValue(
      MMKVUtil.getStringOpt(MMKVKey.regionPreference),
    );
    if (storedPreference != null) return storedPreference;

    final locales = WidgetsBinding.instance.platformDispatcher.locales;
    final countryCode =
        locales.isNotEmpty ? locales.first.countryCode?.toUpperCase() : null;

    return switch (countryCode) {
      'CN' => RegionPreference.mainlandChina,
      'US' => RegionPreference.unitedStates,
      _ => RegionPreference.international,
    };
  }

  String _regionTitle(BuildContext context, RegionPreference region) {
    return switch (region) {
      RegionPreference.mainlandChina =>
        context.tr("privacy.region_mainland_china"),
      RegionPreference.international =>
        context.tr("privacy.region_international"),
      RegionPreference.unitedStates =>
        context.tr("privacy.region_united_states"),
    };
  }

  IconData _regionIcon(RegionPreference region) {
    return switch (region) {
      RegionPreference.mainlandChina => Icons.location_on_outlined,
      RegionPreference.international => Icons.language,
      RegionPreference.unitedStates => Icons.account_balance_outlined,
    };
  }

  Future<void> _openPrivacyPolicy() async {
    await launchUrlString(
      context.tr("privacy.url"),
      mode: LaunchMode.externalApplication,
    );
  }

  Future<void> _showRegionPicker() async {
    final selectedRegion = _selectedRegion;
    if (selectedRegion == null) return;

    final result = await showModalBottomSheet<RegionPreference>(
      context: context,
      backgroundColor: Colors.transparent,
      isScrollControlled: true,
      builder: (context) {
        return _RegionPickerSheet(
          selectedRegion: selectedRegion,
          regionTitle: (region) => _regionTitle(context, region),
          regionIcon: _regionIcon,
        );
      },
    );

    if (result == null || !mounted) return;
    setState(() => _selectedRegion = result);
  }

  void _onContinue() {
    final selectedRegion = _selectedRegion;
    if (selectedRegion == null || !_privacyAccepted) return;

    MMKVUtil.putString(
      MMKVKey.regionPreference,
      selectedRegion.storageValue,
    );
    Navigator.of(context).pop(true);
  }

  void _onDisagree() {
    Navigator.of(context).pop(false);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.75,
      ),
      decoration: const BoxDecoration(
        color: Colors.black,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Center(
              child: CustomPaint(
                size: const Size(40.0, 4.0),
                painter: LinePainter(color: const Color(0xFFB5B5B5)),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 0),
            child: Row(
              children: [
                const SizedBox(width: 48),
                Expanded(
                  child: Text(
                    context.tr("privacy.setup_title"),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
                const SizedBox(width: 48),
              ],
            ),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: Text(
                      context.tr("privacy.setup_desc"),
                      style: const TextStyle(
                        color: Color(0xFFB0B0B0),
                        fontSize: 13,
                        height: 1.35,
                      ),
                    ),
                  ),
                  _SectionTitle(text: context.tr("privacy.region_title")),
                  _RegionPreferenceTile(
                    icon: Icons.public,
                    title: _selectedRegion == null
                        ? context.tr("privacy.region_title")
                        : _regionTitle(context, _selectedRegion!),
                    onTap: _showRegionPicker,
                  ),
                  const SizedBox(height: 2),
                  _SectionTitle(text: context.tr("privacy.name")),
                  _PrivacyAgreementTile(
                    accepted: _privacyAccepted,
                    onChanged: (value) {
                      setState(() => _privacyAccepted = value);
                    },
                    onOpenPrivacyPolicy: _openPrivacyPolicy,
                  ),
                ],
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 10, 20, 20),
            child: Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: _onDisagree,
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.white,
                      side: const BorderSide(color: Color(0xFFB5B5B5)),
                      padding: const EdgeInsets.symmetric(vertical: 12),
                    ),
                    child: Text(context.tr("privacy.disagree_and_exit")),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: FilledButton(
                    onPressed: _canContinue ? _onContinue : null,
                    style: FilledButton.styleFrom(
                      backgroundColor: StyleConstants.defaultColor,
                      foregroundColor: Colors.black,
                      padding: const EdgeInsets.symmetric(vertical: 12),
                    ),
                    child: Text(context.tr("common.continue")),
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

class _SectionTitle extends StatelessWidget {
  const _SectionTitle({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8, top: 4),
      child: Text(
        text,
        style: const TextStyle(
          color: Colors.white,
          fontSize: 15,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

class _RegionPreferenceTile extends StatelessWidget {
  const _RegionPreferenceTile({
    required this.icon,
    required this.title,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          constraints: const BoxConstraints(minHeight: _setupTileMinHeight),
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            color: const Color(0x1AFFFFFF),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Icon(
                icon,
                color: StyleConstants.defaultColor,
                size: 22,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      title,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
              Icon(
                Icons.keyboard_arrow_right,
                color: const Color(0x99FFFFFF),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _RegionPickerSheet extends StatelessWidget {
  const _RegionPickerSheet({
    required this.selectedRegion,
    required this.regionTitle,
    required this.regionIcon,
  });

  final RegionPreference selectedRegion;
  final String Function(RegionPreference region) regionTitle;
  final IconData Function(RegionPreference region) regionIcon;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.55,
      ),
      decoration: const BoxDecoration(
        color: Colors.black,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Center(
              child: CustomPaint(
                size: const Size(40.0, 4.0),
                painter: LinePainter(color: const Color(0xFFB5B5B5)),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 0),
            child: Row(
              children: [
                const SizedBox(width: 48),
                Expanded(
                  child: Text(
                    context.tr("privacy.select_region"),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
                const SizedBox(width: 48),
              ],
            ),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(20, 4, 20, 10),
              child: Column(
                children: [
                  for (final region in RegionPreference.values)
                    _RegionPickerOption(
                      icon: regionIcon(region),
                      title: regionTitle(region),
                      selected: region == selectedRegion,
                      onTap: () => Navigator.of(context).pop(region),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _RegionPickerOption extends StatelessWidget {
  const _RegionPickerOption({
    required this.icon,
    required this.title,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
          decoration: BoxDecoration(
            color: const Color(0x1AFFFFFF),
            borderRadius: BorderRadius.circular(10),
            border: selected
                ? Border.all(color: StyleConstants.defaultColor)
                : Border.all(color: Colors.transparent),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Icon(
                icon,
                color: StyleConstants.defaultColor,
                size: 22,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      title,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ],
                ),
              ),
              Icon(
                selected ? Icons.check_circle : Icons.circle_outlined,
                color: selected
                    ? StyleConstants.defaultColor
                    : const Color(0x99FFFFFF),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PrivacyAgreementTile extends StatelessWidget {
  const _PrivacyAgreementTile({
    required this.accepted,
    required this.onChanged,
    required this.onOpenPrivacyPolicy,
  });

  final bool accepted;
  final ValueChanged<bool> onChanged;
  final VoidCallback onOpenPrivacyPolicy;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: () => onChanged(!accepted),
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            color: const Color(0x1AFFFFFF),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              const Icon(
                Icons.privacy_tip_outlined,
                color: StyleConstants.defaultColor,
                size: 22,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      context.tr("privacy.agreement_title"),
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    Align(
                      alignment: AlignmentDirectional.centerStart,
                      child: TextButton(
                        onPressed: onOpenPrivacyPolicy,
                        style: TextButton.styleFrom(
                          padding: EdgeInsets.zero,
                          minimumSize: Size.zero,
                          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                          foregroundColor: StyleConstants.defaultColor,
                        ),
                        child: Text(
                          context.tr("privacy.view_policy"),
                          style: TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.w600,
                            decoration: TextDecoration.underline,
                            decorationColor: StyleConstants.defaultColor,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              Checkbox(
                value: accepted,
                onChanged: (value) => onChanged(value ?? false),
                activeColor: StyleConstants.defaultColor,
                checkColor: Colors.black,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
