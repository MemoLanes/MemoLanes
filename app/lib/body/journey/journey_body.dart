import 'dart:async';

import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/list/journey_list_calendar.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/body/journey/list/journey_list_empty_state.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/index.dart';
import 'package:memolanes/src/rust/journey_header.dart';

/// Calendar and journey list for the floating map picker.
class JourneyBody extends StatefulWidget {
  const JourneyBody({
    super.key,
    required this.onJourneySelected,
    this.refreshRevision = 0,
  });

  /// Handles the selected journey; the parent owns navigation.
  final Future<void> Function(JourneyHeader journey) onJourneySelected;

  /// Change this value to refresh data using the existing list controller.
  /// The current calendar month, selected date, and journey-kind filters stay
  /// in place when returning from a detail overlay, unless the selected date
  /// falls before the earliest remaining journey.
  final int refreshRevision;

  @override
  State<JourneyBody> createState() => _JourneyBodyState();
}

class _JourneyBodyState extends State<JourneyBody> {
  static const _landscapeContentPadding = 16.0;
  static const _landscapeColumnGap = 16.0;
  static const _landscapeCalendarMinWidth = 320.0;
  static const _landscapeCalendarMaxWidth = 360.0;
  static const _landscapeListMinWidth = 280.0;

  late final JourneyListController _controller;
  final ScrollController _journeyListScrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    _controller = JourneyListController()..addListener(_onControllerChanged);
    _controller.initialize();
  }

  @override
  void didUpdateWidget(covariant JourneyBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.refreshRevision == widget.refreshRevision) return;
    unawaited(
      GlobalLoadingManager.instance.runWithLoading(() => _controller.refresh()),
    );
  }

  @override
  void dispose() {
    _controller
      ..removeListener(_onControllerChanged)
      ..dispose();
    _journeyListScrollController.dispose();
    super.dispose();
  }

  void _onControllerChanged() {
    if (mounted) setState(() {});
  }

  Widget _buildJourneyHeaderList() {
    if (!_controller.hasFilteredJourneys) {
      return JourneyListEmptyState(
        type: JourneyListEmptyType.filtered,
        topAligned: true,
        onShowAll: _showAllJourneyKinds,
        compact: true,
      );
    }
    if (!_controller.hasJourneyOnSelectedDate) {
      return JourneyListEmptyState(
        type: JourneyListEmptyType.month,
        topAligned: true,
        onShowAll: _controller.selectedJourneyKinds.length == 1
            ? _showAllJourneyKinds
            : null,
        compact: true,
      );
    }

    final timeFormat = DateFormat('HH:mm:ss');
    final journeyList = ListView.builder(
      controller: _journeyListScrollController,
      padding: const EdgeInsets.only(right: 12, bottom: 20),
      itemCount: _controller.journeyHeaders.length,
      itemBuilder: (context, index) {
        final header = _controller.journeyHeaders[index];
        return LabelTile(
          label: header.start != null
              ? timeFormat.format(header.start!.toLocal())
              : header.journeyDate.toSimpleDate().toString(),
          prefix: Padding(
            padding: const EdgeInsets.only(right: 12),
            child: Container(
              width: 34,
              height: 34,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: context.appColors.softGreen,
                borderRadius: BorderRadius.circular(11),
              ),
              child: JourneyKindIcon(
                kind: header.journeyKind,
                color: context.appColors.deepGreen,
                size: 18,
              ),
            ),
          ),
          trailing: LabelTileContent(showArrow: true),
          onTap: () async {
            AppHaptics.selection();
            await widget.onJourneySelected(header);
          },
        );
      },
    );

    return ScrollbarTheme(
      data: ScrollbarThemeData(
        thumbColor: WidgetStatePropertyAll(
          context.appColors.deepGreen.withValues(alpha: 0.72),
        ),
        trackColor: WidgetStatePropertyAll(
          context.appColors.softGreen.withValues(alpha: 0.88),
        ),
        trackBorderColor: WidgetStatePropertyAll(context.appColors.lineColor),
      ),
      child: Scrollbar(
        controller: _journeyListScrollController,
        thumbVisibility: true,
        trackVisibility: true,
        interactive: true,
        thickness: 6,
        radius: const Radius.circular(99),
        scrollbarOrientation: ScrollbarOrientation.right,
        child: journeyList,
      ),
    );
  }

  void _showAllJourneyKinds() {
    unawaited(
      GlobalLoadingManager.instance.runWithLoading(
        _controller.showAllJourneyKinds,
      ),
    );
  }

  Widget _buildLandscapeBody(SimpleDate firstDate) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final availableWidth =
            constraints.maxWidth -
            _landscapeContentPadding * 2 -
            _landscapeColumnGap;
        final preferredCalendarWidth = availableWidth * 0.42;
        final maxCalendarWidth = (availableWidth - _landscapeListMinWidth)
            .clamp(0.0, _landscapeCalendarMaxWidth)
            .toDouble();
        final minCalendarWidth = maxCalendarWidth < _landscapeCalendarMinWidth
            ? maxCalendarWidth
            : _landscapeCalendarMinWidth;
        final calendarWidth = preferredCalendarWidth
            .clamp(minCalendarWidth, maxCalendarWidth)
            .toDouble();

        return Padding(
          padding: const EdgeInsets.all(_landscapeContentPadding),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                width: calendarWidth,
                child: SingleChildScrollView(
                  padding: const EdgeInsets.only(bottom: 20),
                  child: _CalendarSurface(
                    child: JourneyListCalendar(
                      controller: _controller,
                      firstDate: firstDate,
                    ),
                  ),
                ),
              ),
              const SizedBox(width: _landscapeColumnGap),
              Expanded(child: _buildJourneyHeaderList()),
            ],
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_controller.isInitialLoading) {
      return const Center(child: CircularProgressIndicator());
    }

    final isLandscape =
        MediaQuery.orientationOf(context) == Orientation.landscape;
    final firstDate = _controller.firstDate;
    if (firstDate == null) {
      return const Padding(
        padding: EdgeInsets.fromLTRB(14, 12, 14, 24),
        child: JourneyListEmptyState(type: JourneyListEmptyType.all),
      );
    }

    if (isLandscape) {
      return _buildLandscapeBody(firstDate);
    }

    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _CalendarSurface(
            child: JourneyListCalendar(
              controller: _controller,
              firstDate: firstDate,
            ),
          ),
          const SizedBox(height: 10),
          Text(
            context.tr('journey.records_title'),
            style: AppTypography.itemTitle.copyWith(
              color: context.appColors.inkColor,
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 6),
          Expanded(child: _buildJourneyHeaderList()),
        ],
      ),
    );
  }
}

class _CalendarSurface extends StatelessWidget {
  const _CalendarSurface({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: context.appColors.surfaceColor,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: context.appColors.lineColor),
      ),
      child: child,
    );
  }
}
