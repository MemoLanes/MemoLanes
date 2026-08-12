import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';

class ImportLoadingPage<T> extends StatefulWidget {
  const ImportLoadingPage({
    super.key,
    required this.filePath,
    required this.load,
    required this.onLoaded,
    required this.onError,
  });

  final String filePath;
  final Future<T> Function() load;
  final Future<void> Function(BuildContext context, T result) onLoaded;
  final Future<void> Function(
    BuildContext context,
    Object error,
    StackTrace stackTrace,
  ) onError;

  @override
  State<ImportLoadingPage<T>> createState() => _ImportLoadingPageState<T>();
}

class _ImportLoadingPageState<T> extends State<ImportLoadingPage<T>> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    try {
      final result = await widget.load();
      if (!mounted) return;
      await widget.onLoaded(context, result);
    } catch (error, stackTrace) {
      if (!mounted) return;
      await widget.onError(context, error, stackTrace);
    }
  }

  @override
  Widget build(BuildContext context) => ImportLoadingScaffold(
        filePath: widget.filePath,
      );
}

class ImportLoadingScaffold extends StatelessWidget {
  const ImportLoadingScaffold({
    super.key,
    required this.filePath,
  });

  final String filePath;

  @override
  Widget build(BuildContext context) {
    final fileName = filePath.split(RegExp(r'[/\\]')).last;
    final fileExtension = fileName.contains('.')
        ? fileName.split('.').last.toUpperCase()
        : context.tr('import.loading.data_file');

    return PopScope(
      canPop: false,
      child: Scaffold(
        appBar: CapsuleStyleAppBar(
          title: context.tr('data.import_data.title'),
        ),
        body: SafeArea(
          top: false,
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 24, 24, 32),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 440),
                child: Column(
                  children: [
                    Text(
                      context.tr('import.loading.title'),
                      style: Theme.of(context).textTheme.titleLarge,
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      context.tr('import.loading.description'),
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: const Color(0x99FFFFFF),
                          ),
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(height: 24),
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: const Color(0x0DFFFFFF),
                        borderRadius: BorderRadius.circular(16),
                      ),
                      child: Row(
                        children: [
                          Container(
                            width: 44,
                            height: 44,
                            decoration: BoxDecoration(
                              color: const Color(0x1AB6E13D),
                              borderRadius: BorderRadius.circular(12),
                            ),
                            child: const Icon(
                              Icons.description_outlined,
                              color: Color(0xFFB6E13D),
                            ),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  fileName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: Theme.of(context).textTheme.bodyMedium,
                                ),
                                const SizedBox(height: 3),
                                Text(
                                  fileExtension,
                                  style: Theme.of(context)
                                      .textTheme
                                      .bodySmall
                                      ?.copyWith(
                                        color: const Color(0x99FFFFFF),
                                      ),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 16),
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: const Color(0x0DFFFFFF),
                        borderRadius: BorderRadius.circular(16),
                      ),
                      child: const ClipRRect(
                        borderRadius: BorderRadius.all(Radius.circular(2)),
                        child: LinearProgressIndicator(
                          minHeight: 3,
                          backgroundColor: Color(0x1AFFFFFF),
                          color: Color(0xFFB6E13D),
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        const Icon(
                          Icons.info_outline,
                          size: 16,
                          color: Color(0x66FFFFFF),
                        ),
                        const SizedBox(width: 6),
                        Flexible(
                          child: Text(
                            context.tr('import.loading.keep_open_hint'),
                            style:
                                Theme.of(context).textTheme.bodySmall?.copyWith(
                                      color: const Color(0x66FFFFFF),
                                    ),
                            textAlign: TextAlign.center,
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
