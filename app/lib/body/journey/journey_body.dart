import 'dart:async';

import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/list/journey_list_calendar.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/body/journey/list/journey_list_empty_state.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/onboarding.dart';
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
  late final JourneyListController _controller;
  final ScrollController _journeyListScrollController = ScrollController();
  // Keep the selected calendar mode/month when its viewport changes shape.
  final _calendarKey = GlobalKey();
  final _firstJourneyRowKey = GlobalKey();
  OnboardingController? _onboarding;
  OnboardingAnchor? _lastGuideAnchor;
  bool _guideScrollScheduled = false;

  bool get _showRecordGuide =>
      _onboarding?.isActive == true &&
      _onboarding?.step.anchor == OnboardingAnchor.journeyRecord;

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
  void didChangeDependencies() {
    super.didChangeDependencies();
    final onboarding = OnboardingScope.maybeOf(context);
    if (_onboarding != onboarding) {
      _onboarding?.removeListener(_onGuideChanged);
      _onboarding = onboarding;
      _onboarding?.addListener(_onGuideChanged);
      _onGuideChanged();
    }
    // Reposition the highlighted row after a rotation or text-size change.
    MediaQuery.of(context);
    _scheduleGuideScroll();
  }

  @override
  void dispose() {
    _onboarding?.removeListener(_onGuideChanged);
    _controller
      ..removeListener(_onControllerChanged)
      ..dispose();
    _journeyListScrollController.dispose();
    super.dispose();
  }

  void _onControllerChanged() {
    if (mounted) setState(() {});
    _scheduleGuideScroll();
  }

  void _onGuideChanged() {
    final anchor = _onboarding?.isActive == true
        ? _onboarding?.step.anchor
        : null;
    if (anchor == _lastGuideAnchor) return;
    _lastGuideAnchor = anchor;
    if (mounted) setState(() {});
    _scheduleGuideScroll();
  }

  void _scheduleGuideScroll() {
    if (_guideScrollScheduled || _onboarding?.isActive != true) return;
    _guideScrollScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _guideScrollScheduled = false;
      if (!mounted || _onboarding?.isActive != true) return;
      if (_showRecordGuide) {
        final rowContext = _firstJourneyRowKey.currentContext;
        if (rowContext != null) {
          unawaited(Scrollable.ensureVisible(rowContext, alignment: 0.55));
        }
      } else if (_onboarding?.step.anchor == OnboardingAnchor.journeys &&
          _journeyListScrollController.hasClients) {
        _journeyListScrollController.jumpTo(0);
      }
    });
  }

  Widget _buildJourneySliver() {
    if (_showRecordGuide &&
        !_controller.isJourneyListLoading &&
        _controller.journeyHeaders.isEmpty) {
      // A first-time user still needs a visible row to learn this interaction.
      // This is a presentation-only example, never a stored JourneyHeader.
      return SliverToBoxAdapter(
        child: Padding(
          padding: const EdgeInsets.only(right: 12, bottom: 20),
          child: _recordTarget(
            LabelTile(
              label: '09:00:00',
              desc: context.tr('onboarding.journey_record.example'),
              descMaxLines: 2,
              prefix: _journeyKindBadge(JourneyKind.defaultKind),
              trailing: const LabelTileContent(showArrow: true),
            ),
          ),
        ),
      );
    }
    if (_controller.firstDate == null) {
      return const SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: EdgeInsets.fromLTRB(14, 12, 14, 24),
          child: JourneyListEmptyState(type: JourneyListEmptyType.all),
        ),
      );
    }
    if (!_controller.hasFilteredJourneys ||
        !_controller.hasJourneyOnSelectedDate) {
      return SliverToBoxAdapter(
        child: JourneyListEmptyState(
          type: _controller.hasFilteredJourneys
              ? JourneyListEmptyType.day
              : JourneyListEmptyType.filtered,
          topAligned: true,
          onShowAll:
              !_controller.hasFilteredJourneys ||
                  _controller.selectedJourneyKinds.length == 1
              ? _showAllJourneyKinds
              : null,
          compact: true,
        ),
      );
    }

    final timeFormat = DateFormat('HH:mm:ss');
    return SliverPadding(
      padding: const EdgeInsets.only(right: 12, bottom: 20),
      sliver: SliverMainAxisGroup(
        slivers: [
          // Build the first row even below a tall calendar, so the guide can
          // scroll to its actual RenderBox without guessing calendar heights.
          if (_controller.journeyHeaders.isNotEmpty)
            SliverToBoxAdapter(child: _buildJourneyRow(context, 0, timeFormat)),
          if (_controller.journeyHeaders.length > 1)
            SliverList.builder(
              itemCount: _controller.journeyHeaders.length - 1,
              itemBuilder: (context, index) =>
                  _buildJourneyRow(context, index + 1, timeFormat),
            ),
        ],
      ),
    );
  }

  Widget _buildJourneyScrollView({Widget? header}) {
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
        child: CustomScrollView(
          controller: _journeyListScrollController,
          slivers: [
            if (header != null) SliverToBoxAdapter(child: header),
            if (header == null && _showRecordGuide)
              SliverToBoxAdapter(child: _recordsHeading()),
            _buildJourneySliver(),
          ],
        ),
      ),
    );
  }

  Widget _buildJourneyRow(
    BuildContext context,
    int index,
    DateFormat timeFormat,
  ) {
    final header = _controller.journeyHeaders[index];
    final tile = LabelTile(
      label: header.start != null
          ? timeFormat.format(header.start!.toLocal())
          : header.journeyDate.toSimpleDate().toString(),
      prefix: _journeyKindBadge(header.journeyKind),
      trailing: const LabelTileContent(showArrow: true),
      onTap: () async {
        AppHaptics.selection();
        await widget.onJourneySelected(header);
      },
    );
    return index == 0 ? _recordTarget(tile) : tile;
  }

  Widget _recordTarget(Widget child) => OnboardingTarget(
    key: _firstJourneyRowKey,
    anchor: OnboardingAnchor.journeyRecord,
    child: child,
  );

  Widget _journeyKindBadge(JourneyKind kind) => Padding(
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
        kind: kind,
        color: context.appColors.deepGreen,
        size: 18,
      ),
    ),
  );

  Widget _calendar(SimpleDate firstDate) => OnboardingTarget(
    anchor: OnboardingAnchor.journeys,
    child: _CalendarSurface(
      child: JourneyListCalendar(
        key: _calendarKey,
        controller: _controller,
        firstDate: firstDate,
        fitLabels: true,
      ),
    ),
  );

  Widget _stackedHeader(SimpleDate firstDate) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _calendar(firstDate),
      const SizedBox(height: 10),
      _recordsHeading(),
    ],
  );

  Widget _recordsHeading() => Padding(
    padding: const EdgeInsets.only(bottom: 6),
    child: Text(
      context.tr('journey.records_title'),
      style: AppTypography.itemTitle.copyWith(
        color: context.appColors.inkColor,
        fontWeight: FontWeight.w700,
      ),
    ),
  );

  void _showAllJourneyKinds() {
    unawaited(
      GlobalLoadingManager.instance.runWithLoading(
        _controller.showAllJourneyKinds,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_controller.isInitialLoading) {
      return const Center(child: CircularProgressIndicator());
    }

    final firstDate = _controller.firstDate;
    if (firstDate == null) return _buildJourneyScrollView();

    return LayoutBuilder(
      builder: (context, constraints) {
        const widePadding = EdgeInsets.all(16);
        // Use the card's clear width, including on portrait inner screens.
        if (constraints.maxWidth - widePadding.horizontal >= 560) {
          return Padding(
            padding: widePadding,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(
                  flex: 11,
                  child: SingleChildScrollView(
                    primary: false,
                    child: _calendar(firstDate),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(flex: 9, child: _buildJourneyScrollView()),
              ],
            ),
          );
        }
        return Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 0),
          // The calendar can leave the viewport instead of squeezing the list.
          child: _buildJourneyScrollView(header: _stackedHeader(firstDate)),
        );
      },
    );
  }
}

class _CalendarSurface extends StatelessWidget {
  const _CalendarSurface({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: context.appColors.surfaceColor,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: context.appColors.lineColor),
      ),
      child: child,
    );
  }
}
