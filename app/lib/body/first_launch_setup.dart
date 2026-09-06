import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_checkbox.dart';
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:url_launcher/url_launcher_string.dart';

const int _latestPrivacyAgreementVersion = 1;
const int _latestFirstLaunchSetupVersion = 1;
const double _setupTileMinHeight = 68.0;

final class _FirstLaunchAccepted {
  const _FirstLaunchAccepted(this.worldview);

  final Worldview worldview;
}

Future<void> _showPrivacyAndRegionSheet(
  BuildContext context, {
  required bool privacyAlreadyAccepted,
}) async {
  if (!privacyAlreadyAccepted) {
    // NOTE: we also use the same mechanism to show public beta testing notice.
    await showCommonDialog(
      context,
      context.tr("beta_testing_notice.content_md"),
      title: context.tr("beta_testing_notice.title"),
      markdown: true,
    );
  }

  // A little weird, but shouldn't happen.
  if (!context.mounted) return;

  final accepted = await showSetupCard<_FirstLaunchAccepted>(
    context,
    barrierDismissible: false,
    builder: (_) =>
        FirstLaunchSetupSheet(initialPrivacyAccepted: privacyAlreadyAccepted),
  );

  if (accepted == null) {
    exit(0);
  }
  MMKVUtil.putInt(
    MMKVKey.privacyAgreementAccepted,
    _latestPrivacyAgreementVersion,
  );
  await showLoadingDialog(
    asyncTask: WorldviewManager.instance.update(accepted.worldview),
  );
  MMKVUtil.putInt(
    MMKVKey.firstLaunchSetupCompletedVersion,
    _latestFirstLaunchSetupVersion,
  );
}

/// Shows the privacy / welcome UI when needed.
Future<void> showFirstLaunchSetupIfNeeded(BuildContext context) async {
  var acceptedVersion = MMKVUtil.getInt(
    MMKVKey.privacyAgreementAccepted,
    defaultValue: 0,
  );
  final privacyAlreadyAccepted =
      acceptedVersion >= _latestPrivacyAgreementVersion;
  final setupCompletedVersion = MMKVUtil.getInt(
    MMKVKey.firstLaunchSetupCompletedVersion,
    defaultValue: 0,
  );
  final firstLaunchSetupAlreadyCompleted =
      setupCompletedVersion >= _latestFirstLaunchSetupVersion;
  if (!privacyAlreadyAccepted || !firstLaunchSetupAlreadyCompleted) {
    await _showPrivacyAndRegionSheet(
      context,
      privacyAlreadyAccepted: privacyAlreadyAccepted,
    );
  }
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
  late Worldview _selectedWorldview;
  late bool _privacyAccepted;

  @override
  void initState() {
    super.initState();
    _privacyAccepted = widget.initialPrivacyAccepted;
    _selectedWorldview = WorldviewManager.instance.currentWorldview;
  }

  Future<void> _openPrivacyPolicy() async {
    await launchUrlString(
      context.tr("privacy.url"),
      mode: LaunchMode.externalApplication,
    );
  }

  Future<void> _showRegionPicker() async {
    final result = await showWorldviewPicker(
      context,
      selectedWorldview: _selectedWorldview,
    );

    if (result == null || !mounted) return;
    setState(() => _selectedWorldview = result);
  }

  void _onContinue() {
    if (!_privacyAccepted) return;
    Navigator.of(context).pop(_FirstLaunchAccepted(_selectedWorldview));
  }

  void _onDisagree() {
    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    return SetupDialogCard(
      title: context.tr("privacy.setup_title"),
      maxHeightFactor: 0.75,
      actions: [
        AppButton(
          label: context.tr("privacy.disagree_and_exit"),
          labelMaxLines: 2,
          onPressed: _onDisagree,
          variant: AppButtonVariant.secondary,
        ),
        AppButton(
          label: context.tr("common.continue"),
          labelMaxLines: 2,
          onPressed: _privacyAccepted ? _onContinue : null,
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: 10),
            child: Text(
              context.tr("privacy.setup_desc"),
              style: AppTypography.supporting.copyWith(
                color: StyleConstants.mutedInkColor,
              ),
            ),
          ),
          _SectionTitle(text: context.tr("privacy.region_title")),
          SetupTile(
            icon: Icons.public,
            title: regionPreferenceTitle(context, _selectedWorldview),
            onTap: _showRegionPicker,
            minHeight: _setupTileMinHeight,
            trailing: Icon(
              Icons.keyboard_arrow_right,
              color: StyleConstants.mutedInkColor,
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
        style: AppTypography.cardTitle.copyWith(
          color: StyleConstants.deepGreen,
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
            foregroundColor: StyleConstants.deepGreen,
          ),
          child: Text(
            context.tr("privacy.view_policy"),
            style: AppTypography.supporting.copyWith(
              fontWeight: FontWeight.w600,
              decoration: TextDecoration.underline,
              decorationColor: StyleConstants.deepGreen,
            ),
          ),
        ),
      ),
      trailing: AppCheckbox(value: accepted, onChanged: onChanged),
    );
  }
}
