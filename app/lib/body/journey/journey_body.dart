import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
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
import 'package:memolanes/utils/nav_helper.dart';

class JourneyBody extends StatefulWidget {
  const JourneyBody({
    super.key,
    this.onJourneySelected,
    this.compactPicker = false,
    this.refreshRevision = 0,
  });

  /// When supplied, tapping a row selects it instead of opening the details
  /// page. This lets the calendar/list work inside the map picker.
  final Future<void> Function(JourneyHeader journey)? onJourneySelected;

  /// Removes the full-page heading and tightens spacing for the floating map
  /// picker. The calendar remains above a separately scrollable journey list.
  final bool compactPicker;

  /// Refreshes the existing list controller without resetting the selected
  /// month, date, or journey-kind filters.
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

  bool get _isSelectionMode => widget.onJourneySelected != null;
  bool get _isCompactPicker => _isSelectionMode && widget.compactPicker;

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
      GlobalLoadingManager.instance.runWithLoading(
        () => _controller.refresh(adjustSelectedDate: true),
      ),
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
        compact: _isCompactPicker,
      );
    }
    if (!_controller.hasJourneyOnSelectedDate) {
      return JourneyListEmptyState(
        type: JourneyListEmptyType.month,
        topAligned: true,
        onShowAll: _controller.selectedJourneyKinds.length == 1
            ? _showAllJourneyKinds
            : null,
        compact: _isCompactPicker,
      );
    }

    final timeFormat = DateFormat('HH:mm:ss');
    final journeyList = ListView.builder(
      controller: _journeyListScrollController,
      padding: EdgeInsets.only(
        right: _isCompactPicker ? 12 : 0,
        bottom: _isSelectionMode ? 20 : StyleConstants.navBarSafeArea + 5,
      ),
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
                color: StyleConstants.softGreen,
                borderRadius: BorderRadius.circular(11),
              ),
              child: JourneyKindIcon(
                kind: header.journeyKind,
                color: StyleConstants.deepGreen,
                size: 18,
              ),
            ),
          ),
          trailing: LabelTileContent(showArrow: true),
          onTap: () async {
            final onJourneySelected = widget.onJourneySelected;
            if (onJourneySelected != null) {
              AppHaptics.selection();
              await onJourneySelected(header);
              return;
            }
            if (!context.mounted) return;
            navigatorPush(
              context,
              page: JourneyInfoPage(journeyHeader: header),
            ).then((refresh) async {
              if (refresh == true) {
                await GlobalLoadingManager.instance.runWithLoading(
                  () => _controller.refresh(adjustSelectedDate: true),
                );
              }
            });
          },
        );
      },
    );

    if (!_isCompactPicker) return journeyList;

    return ScrollbarTheme(
      data: ScrollbarThemeData(
        thumbColor: WidgetStatePropertyAll(
          StyleConstants.deepGreen.withValues(alpha: 0.72),
        ),
        trackColor: WidgetStatePropertyAll(
          StyleConstants.softGreen.withValues(alpha: 0.88),
        ),
        trackBorderColor: WidgetStatePropertyAll(StyleConstants.lineColor),
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
    final bottomPadding = _isSelectionMode
        ? 20.0
        : StyleConstants.navBarSafeArea + 5;
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
                  padding: EdgeInsets.only(bottom: bottomPadding),
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
      return Padding(
        padding: EdgeInsets.fromLTRB(
          _isCompactPicker ? 14 : 8,
          _isCompactPicker ? 12 : 16,
          _isCompactPicker ? 14 : 8,
          _isSelectionMode
              ? 24
              : (isLandscape ? StyleConstants.navBarSafeArea + 5 : 140),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (!_isCompactPicker)
              _JourneyPageHeader(selectionMode: _isSelectionMode),
            const Expanded(
              child: JourneyListEmptyState(type: JourneyListEmptyType.all),
            ),
          ],
        ),
      );
    }

    if (isLandscape) {
      return _buildLandscapeBody(firstDate);
    }

    return Padding(
      padding: EdgeInsets.fromLTRB(
        _isCompactPicker ? 12 : 8,
        _isCompactPicker ? 10 : 16,
        _isCompactPicker ? 12 : 8,
        0,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (!_isCompactPicker) ...[
            _JourneyPageHeader(selectionMode: _isSelectionMode),
            const SizedBox(height: 20),
          ],
          _CalendarSurface(
            child: JourneyListCalendar(
              controller: _controller,
              firstDate: firstDate,
            ),
          ),
          SizedBox(height: _isCompactPicker ? 10 : 16),
          Text(
            context.tr('journey.records_title'),
            style:
                (_isCompactPicker
                        ? AppTypography.itemTitle
                        : AppTypography.subpageTitle)
                    .copyWith(
                      color: StyleConstants.inkColor,
                      fontWeight: FontWeight.w700,
                    ),
          ),
          SizedBox(height: _isCompactPicker ? 6 : 10),
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
        color: StyleConstants.surfaceColor,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: StyleConstants.lineColor),
      ),
      child: child,
    );
  }
}

class _JourneyPageHeader extends StatelessWidget {
  const _JourneyPageHeader({required this.selectionMode});

  final bool selectionMode;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.tr(
            selectionMode
                ? 'journey.picker_title'
                : 'journey.editor_overview_title',
          ),
          style:
              (selectionMode
                      ? AppTypography.compactPageTitle
                      : AppTypography.pageTitle)
                  .copyWith(
                    color: StyleConstants.inkColor,
                    fontWeight: FontWeight.w700,
                    height: 1,
                  ),
        ),
        if (selectionMode) ...[
          const SizedBox(height: 7),
          Text(
            context.tr('journey.picker_subtitle'),
            style: AppTypography.body.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
        ],
      ],
    );
  }
}
