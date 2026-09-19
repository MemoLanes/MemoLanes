import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class CompactJourneyTimeDialog extends StatefulWidget {
  const CompactJourneyTimeDialog({super.key, required this.initialTime});

  final TimeOfDay initialTime;

  @override
  State<CompactJourneyTimeDialog> createState() =>
      _CompactJourneyTimeDialogState();
}

class _CompactJourneyTimeDialogState extends State<CompactJourneyTimeDialog> {
  late int _hour;
  late int _minute;
  late bool _isPm;
  late final FixedExtentScrollController _hour24Controller;
  late final FixedExtentScrollController _hour12Controller;
  late final FixedExtentScrollController _minuteController;
  late final FixedExtentScrollController _periodController;

  @override
  void initState() {
    super.initState();
    _hour = widget.initialTime.hour;
    _minute = widget.initialTime.minute;
    _isPm = _hour >= 12;
    final displayHour = _hour % 12 == 0 ? 12 : _hour % 12;
    _hour24Controller = FixedExtentScrollController(initialItem: _hour);
    _hour12Controller = FixedExtentScrollController(
      initialItem: displayHour - 1,
    );
    _minuteController = FixedExtentScrollController(initialItem: _minute);
    _periodController = FixedExtentScrollController(initialItem: _isPm ? 1 : 0);
  }

  @override
  void dispose() {
    _hour24Controller.dispose();
    _hour12Controller.dispose();
    _minuteController.dispose();
    _periodController.dispose();
    super.dispose();
  }

  Widget _wheel({
    required FixedExtentScrollController controller,
    required int itemCount,
    required int selectedIndex,
    required String Function(int index) labelBuilder,
    required ValueChanged<int> onSelectedItemChanged,
    double width = 80,
  }) {
    return Flexible(
      child: SizedBox(
        width: width,
        child: Stack(
          alignment: Alignment.center,
          children: [
            IgnorePointer(
              child: Container(
                height: 40,
                decoration: BoxDecoration(
                  color: StyleConstants.softGreen.withValues(alpha: 0.82),
                  borderRadius: BorderRadius.circular(10),
                ),
              ),
            ),
            CupertinoPicker(
              scrollController: controller,
              itemExtent: 40,
              diameterRatio: 1.45,
              squeeze: 1.08,
              selectionOverlay: const SizedBox.shrink(),
              onSelectedItemChanged: onSelectedItemChanged,
              children: List.generate(
                itemCount,
                (index) => Center(
                  child: Text(
                    labelBuilder(index),
                    style: AppTypography.pickerValue.copyWith(
                      color: index == selectedIndex
                          ? StyleConstants.deepGreen
                          : StyleConstants.mutedInkColor.withValues(
                              alpha: 0.58,
                            ),
                      fontWeight: index == selectedIndex
                          ? FontWeight.w700
                          : FontWeight.w600,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final localizations = MaterialLocalizations.of(context);
    final use24HourFormat = MediaQuery.alwaysUse24HourFormatOf(context);
    final selectedHourIndex = use24HourFormat
        ? _hour
        : ((_hour % 12 == 0 ? 12 : _hour % 12) - 1);

    return Dialog(
      elevation: 0,
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 24),
      child: PointerInterceptor(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 390),
          child: JourneyInfoPanelSurface(
            child: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    SizedBox(
                      width: double.infinity,
                      height: 290,
                      child: Column(
                        children: [
                          const SizedBox(height: 8),
                          Text(
                            localizations.timePickerDialHelpText,
                            style: AppTypography.surfaceTitle.copyWith(
                              color: StyleConstants.deepGreen,
                            ),
                          ),
                          const SizedBox(height: 12),
                          Expanded(
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                _wheel(
                                  controller: use24HourFormat
                                      ? _hour24Controller
                                      : _hour12Controller,
                                  itemCount: use24HourFormat ? 24 : 12,
                                  selectedIndex: selectedHourIndex,
                                  labelBuilder: (index) => use24HourFormat
                                      ? index.toString().padLeft(2, '0')
                                      : '${index + 1}',
                                  onSelectedItemChanged: (index) {
                                    AppHaptics.selection();
                                    setState(() {
                                      if (use24HourFormat) {
                                        _hour = index;
                                      } else {
                                        final hour12 = index + 1;
                                        _hour = hour12 % 12 + (_isPm ? 12 : 0);
                                      }
                                    });
                                  },
                                ),
                                Padding(
                                  padding: EdgeInsets.symmetric(horizontal: 5),
                                  child: Text(
                                    ':',
                                    style: AppTypography.metricTitle.copyWith(
                                      color: StyleConstants.deepGreen,
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                                ),
                                _wheel(
                                  controller: _minuteController,
                                  itemCount: 60,
                                  selectedIndex: _minute,
                                  labelBuilder: (index) =>
                                      index.toString().padLeft(2, '0'),
                                  onSelectedItemChanged: (index) {
                                    AppHaptics.selection();
                                    setState(() => _minute = index);
                                  },
                                ),
                                if (!use24HourFormat) ...[
                                  const SizedBox(width: 10),
                                  _wheel(
                                    controller: _periodController,
                                    itemCount: 2,
                                    selectedIndex: _isPm ? 1 : 0,
                                    width: 64,
                                    labelBuilder: (index) => index == 0
                                        ? localizations.anteMeridiemAbbreviation
                                        : localizations
                                              .postMeridiemAbbreviation,
                                    onSelectedItemChanged: (index) {
                                      AppHaptics.selection();
                                      setState(() {
                                        _isPm = index == 1;
                                        _hour = _hour % 12 + (_isPm ? 12 : 0);
                                      });
                                    },
                                  ),
                                ],
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 6),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        SizedBox(
                          width: 112,
                          child: AppButton(
                            size: AppButtonSize.compact,
                            expand: true,
                            icon: Icons.close_rounded,
                            label: localizations.cancelButtonLabel,
                            onPressed: () => Navigator.of(context).pop(),
                          ),
                        ),
                        const SizedBox(width: 8),
                        SizedBox(
                          width: 112,
                          child: AppButton(
                            size: AppButtonSize.compact,
                            expand: true,
                            icon: Icons.check_rounded,
                            label: localizations.okButtonLabel,
                            variant: AppButtonVariant.primary,
                            onPressed: () => Navigator.of(context)
                                .pop(TimeOfDay(hour: _hour, minute: _minute)),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
