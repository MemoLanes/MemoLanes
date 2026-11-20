import 'dart:async';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';

import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';

import 'tracelet_motion_diagnostics.dart';

class TraceletMotionDebugPage extends StatefulWidget {
  const TraceletMotionDebugPage({super.key});

  @override
  State<TraceletMotionDebugPage> createState() =>
      _TraceletMotionDebugPageState();
}

class _TraceletMotionDebugPageState extends State<TraceletMotionDebugPage>
    with WidgetsBindingObserver {
  static const _key = 'general.advanced_settings.debug.motion';
  final _diagnostics = TraceletMotionDiagnostics.instance;
  late final Future<void> _attachment;
  late Future<List<File>> _logs = _loadLogs();
  bool _busy = false;
  bool _actionFailed = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _attachment = _diagnostics.attach();
    unawaited(_refreshMotion());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    unawaited(
      _attachment.whenComplete(_diagnostics.detach).catchError((
        Object error,
        StackTrace stackTrace,
      ) {
        log.error(
          '[Tracelet motion] Could not detach diagnostics: $error',
          stackTrace,
        );
      }),
    );
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) unawaited(_refreshMotion());
  }

  Future<void> _refreshMotion() async {
    try {
      await _attachment;
      if (!mounted) return;
      await _diagnostics.refreshMotionState();
    } catch (error, stackTrace) {
      _reportError('read the current motion state', error, stackTrace);
    }
  }

  Future<List<File>> _loadLogs() async {
    try {
      return await _diagnostics.savedLogs();
    } catch (error, stackTrace) {
      log.error('[Tracelet motion] Could not list logs: $error', stackTrace);
      rethrow;
    }
  }

  void _reportError(String action, Object error, StackTrace stackTrace) {
    log.error('[Tracelet motion] Could not $action: $error', stackTrace);
    if (mounted) setState(() => _actionFailed = true);
  }

  Future<void> _run(String action, Future<void> Function() operation) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _actionFailed = false;
    });
    try {
      await _attachment;
      await operation();
      if (mounted) {
        setState(() {
          _logs = _loadLogs();
        });
      }
    } catch (error, stackTrace) {
      _reportError(action, error, stackTrace);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _toggleCapture() {
    return _diagnostics.capturing
        ? _run('stop capture', _diagnostics.stopCapture)
        : _run('start capture', _diagnostics.startCapture);
  }

  Future<void> _deleteLog(File file) => _run('delete the log', () async {
    final confirmed = await showCommonDialog(
      context,
      file.uri.pathSegments.last,
      title: context.tr('common.delete'),
      hasCancel: true,
      confirmButtonText: context.tr('common.delete'),
      confirmVariant: AppButtonVariant.danger,
    );
    if (confirmed) await file.delete();
  });

  Widget _savedLogs() => FutureBuilder<List<File>>(
    future: _logs,
    builder: (context, snapshot) {
      if (snapshot.hasError) return Text(context.tr('$_key.logs_unavailable'));
      final files = snapshot.data;
      if (files == null) return const LinearProgressIndicator();
      if (files.isEmpty) return Text(context.tr('$_key.no_logs'));
      final disabled = _busy || _diagnostics.capturing;
      return Column(
        children: [
          for (final file in files)
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.ios_share),
              title: Text(file.uri.pathSegments.last),
              onTap: disabled
                  ? null
                  : () => showCommonExport(context, file.path),
              trailing: IconButton(
                tooltip: context.tr('common.delete'),
                onPressed: disabled ? null : () => _deleteLog(file),
                icon: const Icon(Icons.delete_outline),
              ),
            ),
        ],
      );
    },
  );

  Widget _callbackDetails(BuildContext context, TraceletMotionFix fix) {
    final sourceKey = switch (fix.source) {
      'gps' => 'source_gps',
      'wifi' => 'source_wifi',
      'cell' => 'source_cell',
      'network' => 'source_network',
      _ => 'source_unknown',
    };
    final clock = DateFormat('HH:mm:ss.SSS');
    String milliseconds(int? value) => value == null ? '—' : '$value ms';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.tr(
            '$_key.latest_callback_details',
            namedArgs: {
              'count': '${_diagnostics.fixCount}',
              'source': context.tr('$_key.$sourceKey'),
              'fixTime': clock.format(
                DateTime.fromMillisecondsSinceEpoch(fix.timestampMs),
              ),
              'receivedTime': clock.format(fix.receivedAt),
              'fixInterval': milliseconds(fix.fixIntervalMs),
              'callbackInterval': milliseconds(fix.callbackIntervalMs),
              'age': milliseconds(fix.fixAgeMs),
              'speed': fix.speed == null
                  ? '—'
                  : '${fix.speed!.toStringAsFixed(1)} m/s',
              'accuracy': '${fix.accuracy.toStringAsFixed(1)} m',
            },
          ),
        ),
        if (fix.isMock) Text(context.tr('$_key.mock_fix')),
        if (fix.reducedAccuracy) Text(context.tr('$_key.reduced_accuracy_fix')),
        if (Platform.isIOS)
          Text(
            context.tr('$_key.ios_source_estimate'),
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: CapsuleStyleAppBar(title: context.tr('$_key.title')),
    body: ListenableBuilder(
      listenable: _diagnostics,
      builder: (context, _) {
        final motionKey = switch (_diagnostics.isMoving) {
          true => 'moving',
          false => 'stationary',
          null => 'idle',
        };
        final capturing = _diagnostics.capturing;
        final recent = _diagnostics.recentLines;
        return SettingsPageLayout(
          children: [
            SettingsTile(
              label: context.tr('$_key.motion_state'),
              trailing: LabelTileContent(
                content: context.tr('$_key.$motionKey'),
              ),
            ),
            const SizedBox(height: 8),
            if (_diagnostics.lastFix case final fix?)
              _callbackDetails(context, fix),
            if (_diagnostics.isMoving == null)
              Text(context.tr('$_key.idle_hint')),
            const SizedBox(height: 16),
            AppButton(
              label: context.tr(
                '$_key.${capturing ? 'stop_capture' : 'start_capture'}',
              ),
              onPressed: _busy ? null : _toggleCapture,
              loading: _busy,
              expand: true,
            ),
            Text(
              context.tr('$_key.${capturing ? 'capturing' : 'capture_hint'}'),
            ),
            if (_actionFailed) Text(context.tr('$_key.action_failed')),
            if (_diagnostics.writeError != null)
              Text(context.tr('$_key.write_failed')),
            if (recent.isNotEmpty) ...[
              const Divider(),
              Text(context.tr('$_key.recent_events')),
              SizedBox(
                height: 180,
                child: ListView.builder(
                  primary: false,
                  reverse: true,
                  itemCount: recent.length,
                  itemBuilder: (context, index) => Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: SelectableText(
                      recent[recent.length - index - 1],
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                      ),
                    ),
                  ),
                ),
              ),
            ],
            const Divider(),
            Text(context.tr('$_key.saved_logs')),
            _savedLogs(),
          ],
        );
      },
    ),
  );
}
