import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/journey_body.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/body/map/overlay/normal_map_overlay.dart';
import 'package:memolanes/body/onboarding/onboarding_controller.dart';
import 'package:memolanes/body/onboarding/onboarding_overlay.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/component/recording_buttons.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';

Future<void> showGettingStarted(
  BuildContext context, {
  Listenable? interruptOn,
}) async {
  if (await const OnboardingPage().show(context, interruptOn: interruptOn)) {
    MMKVUtil.putBool(MMKVKey.onboardingCompleted, true);
  }
}

/// Owns the tour and previews independently of the live home tab and map.
class OnboardingPage extends StatefulWidget {
  const OnboardingPage({super.key, this.map = const _PreviewMap()});

  final Widget map;

  Future<bool> show(BuildContext context, {Listenable? interruptOn}) async {
    if (!context.mounted || ModalRoute.isCurrentOf(context) != true) {
      return false;
    }
    final navigator = Navigator.of(context, rootNavigator: true);
    final route = MaterialPageRoute<bool>(
      fullscreenDialog: true,
      builder: (_) => this,
    );
    final result = navigator.push(route);
    void interrupt() {
      if (route.isActive) navigator.removeRoute(route, false);
    }

    interruptOn?.addListener(interrupt);
    try {
      return await result == true;
    } finally {
      interruptOn?.removeListener(interrupt);
    }
  }

  @override
  State<OnboardingPage> createState() => _OnboardingPageState();
}

class _OnboardingPageState extends State<OnboardingPage> {
  final _recordingKey = GlobalKey();
  final _mapControlsKey = GlobalKey();
  final _timeRangeKey = GlobalKey();
  final _calendarKey = GlobalKey();
  final _recordKey = GlobalKey();
  final _journeyScroll = ScrollController();
  late final _guide = OnboardingController(finish: _finish);
  late final _sampleDate = SimpleDate.today();
  bool _scrollScheduled = false;

  void _finish() {
    if (ModalRoute.isCurrentOf(context) == true) {
      Navigator.of(context).pop(true);
    }
  }

  @override
  void dispose() {
    _guide.dispose();
    _journeyScroll.dispose();
    super.dispose();
  }

  void _revealJourneyTarget() {
    if (_scrollScheduled) return;
    _scrollScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _scrollScheduled = false;
      if (!mounted || _guide.step.tab != 2 || !_journeyScroll.hasClients) {
        return;
      }
      final position = _journeyScroll.position;
      if (!position.hasContentDimensions) return;
      // shortcut: one sample row, use its anchor if the preview adds rows.
      final offset = _guide.step == OnboardingStep.journeyRecord
          ? position.maxScrollExtent
          : position.minScrollExtent;
      if (offset != position.pixels) position.jumpTo(offset);
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  JourneyListController _sampleJourneys(BuildContext context) {
    final start = DateTime(
      _sampleDate.year,
      _sampleDate.month,
      _sampleDate.day,
      9,
    );
    return JourneyListController.inMemory([
      JourneyHeader(
        id: 'onboarding-example',
        revision: 'preview',
        journeyDate: _sampleDate.toFrbNaiveDate(),
        createdAt: start,
        start: start,
        journeyType: JourneyType.vector,
        journeyKind: JourneyKind.defaultKind,
        note: context.tr('onboarding.journey_record.example'),
        hasRawData: false,
      ),
    ]);
  }

  Widget _buildControls(BuildContext context, OnboardingStep step) {
    return switch (step.tab) {
      0 => NormalMapOverlay(
        trackingMode: TrackingMode.off,
        onTrackingPressed: () {},
        recordingControls: step == OnboardingStep.recording
            ? StartJourneyButton(key: _recordingKey)
            : null,
        mapControlsKey: _mapControlsKey,
      ),
      1 => MapOverlayPanel.controls(
        child: TimeRangePicker(
          key: _timeRangeKey,
          earliestDate: SimpleDate(_sampleDate.year),
          selectedJourneyKinds: const {
            JourneyKind.defaultKind,
            JourneyKind.flight,
          },
          onRangeChanged: (_, _) {},
        ),
      ),
      2 => NotificationListener<ScrollMetricsNotification>(
        onNotification: (_) {
          _revealJourneyTarget();
          return false;
        },
        child: JourneyPickerOverlay(
          child: JourneyBody(
            onJourneySelected: null,
            showNotes: true,
            createController: () => _sampleJourneys(context),
            scrollController: _journeyScroll,
            calendarKey: _calendarKey,
            firstRecordKey: _recordKey,
          ),
        ),
      ),
      _ => const SizedBox.shrink(),
    };
  }

  @override
  Widget build(BuildContext context) => PopScope<bool>(
    canPop: false,
    onPopInvokedWithResult: (didPop, _) {
      if (!didPop) _guide.back();
    },
    child: ListenableBuilder(
      listenable: _guide,
      builder: (context, _) {
        final step = _guide.step;
        if (step.tab == 2) _revealJourneyTarget();
        final target = switch (step) {
          OnboardingStep.recording => _recordingKey,
          OnboardingStep.mapControls => _mapControlsKey,
          OnboardingStep.timeRange => _timeRangeKey,
          OnboardingStep.journeys => _calendarKey,
          OnboardingStep.journeyRecord => _recordKey,
          _ => null,
        };
        return AppNavigationShell(
          navigationBuilder: (_, {required axis}) => ExcludeFocus(
            child: BottomNavBar(
              selectedIndex: step.tab,
              onIndexChanged: (_) {},
              hasUpdateNotification: () => false,
              axis: axis,
            ),
          ),
          bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
              ExcludeFocus(
                child: OnboardingTarget(
                  controller: _guide,
                  target: target,
                  geometryChanges: _journeyScroll,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [widget.map, _buildControls(context, step)],
                  ),
                ),
              ),
          foregroundOverlay: OnboardingOverlay(controller: _guide),
        );
      },
    ),
  );
}

class _PreviewMap extends StatefulWidget {
  const _PreviewMap();

  @override
  State<_PreviewMap> createState() => _PreviewMapState();
}

class _PreviewMapState extends State<_PreviewMap> {
  late final _renderer = api.getEmptyMapRendererProxy();

  @override
  Widget build(BuildContext context) =>
      BaseMapWebview(mapRendererProxy: _renderer);
}
