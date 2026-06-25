import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:url_launcher/url_launcher_string.dart';

const int _latestVersion = 1;
const double _setupTileMinHeight = 68.0;

Future<void> _showPrivacyAndRegionSheet(
  BuildContext context, {
  required bool privacyAlreadyAccepted,
}) async {
  if (!privacyAlreadyAccepted) {
    // NOTE: we also use the same mechanism to show public beta testing notice.
    await showCommonDialog(
        context, context.tr("beta_testing_notice.content_md"),
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
      return FirstLaunchSetupSheet(
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
Future<void> showFirstLaunchSetupIfNeeded(BuildContext context) {
  var acceptedVersion =
      MMKVUtil.getInt(MMKVKey.privacyAgreementAccepted, defaultValue: 0);
  final privacyAlreadyAccepted = acceptedVersion >= _latestVersion;
  final regionPreferenceSelected = MMKVUtil.getBool(
    MMKVKey.regionPreferenceSelected,
    defaultValue: false,
  );
  if (!privacyAlreadyAccepted || !regionPreferenceSelected) {
    return _showPrivacyAndRegionSheet(
      context,
      privacyAlreadyAccepted: privacyAlreadyAccepted,
    );
  }
  return Future.value();
}

class FirstLaunchSetupSheet extends StatefulWidget {
  const FirstLaunchSetupSheet({
    super.key,
    required this.initialPrivacyAccepted,
  });

  final bool initialPrivacyAccepted;

  @override
  State<FirstLaunchSetupSheet> createState() => _FirstLaunchSetupSheetState();
}

class _FirstLaunchSetupSheetState extends State<FirstLaunchSetupSheet> {
  late RegionPreference _selectedRegion;
  late bool _privacyAccepted;
  bool _saving = false;

  bool get _canContinue => _privacyAccepted && !_saving;

  @override
  void initState() {
    super.initState();
    _privacyAccepted = widget.initialPrivacyAccepted;
    _selectedRegion = defaultRegionPreferenceFromDeviceLocale();
  }

  Future<void> _openPrivacyPolicy() async {
    await launchUrlString(
      context.tr("privacy.url"),
      mode: LaunchMode.externalApplication,
    );
  }

  Future<void> _showRegionPicker() async {
    final result = await showRegionPreferencePicker(
      context,
      selectedRegion: _selectedRegion,
    );

    if (result == null || !mounted) return;
    setState(() => _selectedRegion = result);
  }

  Future<void> _onContinue() async {
    if (!_privacyAccepted) return;

    setState(() => _saving = true);
    try {
      await saveRegionPreference(_selectedRegion);
      MMKVUtil.putBool(MMKVKey.regionPreferenceSelected, true);
    } catch (_) {
      if (!mounted) return;
      await showCommonDialog(context, context.tr("privacy.region_save_failed"));
      return;
    } finally {
      if (mounted) {
        setState(() => _saving = false);
      }
    }
    if (!mounted) return;
    Navigator.of(context).pop(true);
  }

  void _onDisagree() {
    Navigator.of(context).pop(false);
  }

  @override
  Widget build(BuildContext context) {
    return SetupBottomSheet(
      title: context.tr("privacy.setup_title"),
      maxHeightFactor: 0.75,
      actions: [
        OutlinedButton(
          onPressed: _onDisagree,
          style: OutlinedButton.styleFrom(
            foregroundColor: Colors.white,
            side: const BorderSide(color: Color(0xFFB5B5B5)),
            padding: const EdgeInsets.symmetric(vertical: 12),
          ),
          child: Text(context.tr("privacy.disagree_and_exit")),
        ),
        FilledButton(
          onPressed: _canContinue ? _onContinue : null,
          style: FilledButton.styleFrom(
            backgroundColor: StyleConstants.defaultColor,
            foregroundColor: Colors.black,
            padding: const EdgeInsets.symmetric(vertical: 12),
          ),
          child: Text(context.tr("common.continue")),
        ),
      ],
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
          SetupTile(
            icon: Icons.public,
            title: regionPreferenceTitle(context, _selectedRegion),
            onTap: _showRegionPicker,
            minHeight: _setupTileMinHeight,
            trailing: const Icon(
              Icons.keyboard_arrow_right,
              color: Color(0x99FFFFFF),
            ),
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
    return SetupTile(
      icon: Icons.privacy_tip_outlined,
      title: context.tr("privacy.agreement_title"),
      onTap: () => onChanged(!accepted),
      extraContent: Align(
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
      trailing: Checkbox(
        value: accepted,
        onChanged: (value) => onChanged(value ?? false),
        activeColor: StyleConstants.defaultColor,
        checkColor: Colors.black,
      ),
    );
  }
}
