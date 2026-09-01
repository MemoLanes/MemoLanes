import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/body/journey/list/journey_list_calendar.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/body/journey/list/journey_list_empty_state.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/index.dart';
import 'package:memolanes/utils/nav_helper.dart';

class JourneyBody extends StatefulWidget {
  const JourneyBody({super.key});

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
      );
    }
    if (!_controller.hasJourneyOnSelectedDate) {
      return JourneyListEmptyState(
        type: JourneyListEmptyType.month,
        topAligned: true,
        onShowAll: _controller.selectedJourneyKinds.length == 1
            ? _showAllJourneyKinds
            : null,
      );
    }

    final timeFormat = DateFormat('HH:mm:ss');
    final journeyList = ListView.builder(
      controller: _journeyListScrollController,
      padding: EdgeInsets.only(bottom: StyleConstants.navBarSafeArea + 5),
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
          onTap: () {
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

    return journeyList;
  }

  void _showAllJourneyKinds() {
    unawaited(
      GlobalLoadingManager.instance.runWithLoading(
        _controller.showAllJourneyKinds,
      ),
    );
  }

  Widget _buildLandscapeBody(SimpleDate firstDate) {
    const bottomPadding = StyleConstants.navBarSafeArea + 5;
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

    final firstDate = _controller.firstDate;
    if (firstDate == null) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(20, 28, 20, 140),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const _JourneyPageHeader(),
            const Expanded(
              child: JourneyListEmptyState(type: JourneyListEmptyType.all),
            ),
          ],
        ),
      );
    }

    if (MediaQuery.of(context).orientation == Orientation.landscape) {
      return _buildLandscapeBody(firstDate);
    }

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 22, 16, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _JourneyPageHeader(),
          const SizedBox(height: 18),
          _CalendarSurface(
            child: JourneyListCalendar(
              controller: _controller,
              firstDate: firstDate,
            ),
          ),
          const SizedBox(height: 16),
          Text(
            context.tr('journey.records_title'),
            style: AppTypography.subpageTitle.copyWith(
              color: StyleConstants.inkColor,
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 10),
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
  const _JourneyPageHeader();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.tr('journey.editor_overview_title'),
          style: AppTypography.pageTitle.copyWith(
            color: StyleConstants.inkColor,
          ),
        ),
        const SizedBox(height: 7),
        Text(
          context.tr('journey.editor_overview_subtitle'),
          style: AppTypography.body.copyWith(
            color: StyleConstants.mutedInkColor,
          ),
        ),
      ],
    );
  }
}
