import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/utils.dart';

// The delete result is returned only after the user confirms deletion.
enum JourneyMoreAction { delete }

Future<JourneyMoreAction?> showJourneyMoreDialog(BuildContext context) {
  return showAppDialog<JourneyMoreAction>(
    context,
    maxWidth: 360,
    insetPadding: const EdgeInsets.symmetric(horizontal: 38, vertical: 24),
    builder: (dialogContext) {
      return AppDialogCard(
        title: dialogContext.tr('common.more'),
        surfaceStyle: AppDialogSurfaceStyle.glass,
        maxHeightFactor: 0.6,
        contentPadding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            AppButton(
              expand: true,
              icon: Icons.delete_outline_rounded,
              label: dialogContext.tr('journey.delete_journey_title'),
              variant: AppButtonVariant.danger,
              onPressed: () async {
                if (!dialogContext.mounted ||
                    ModalRoute.of(dialogContext)?.isCurrent != true) {
                  return;
                }
                // Keep this menu underneath the confirmation so Cancel
                // (or system Back) returns to the available actions.
                final shouldDelete = await showCommonDialog(
                  dialogContext,
                  dialogContext.tr('journey.delete_journey_message'),
                  hasCancel: true,
                  title: dialogContext.tr('journey.delete_journey_title'),
                  confirmButtonText: dialogContext.tr('common.delete'),
                  confirmVariant: AppButtonVariant.danger,
                );
                if (shouldDelete && dialogContext.mounted) {
                  popCurrentRoute(dialogContext, JourneyMoreAction.delete);
                }
              },
            ),
          ],
        ),
      );
    },
  );
}
