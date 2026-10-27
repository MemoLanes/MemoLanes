import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';
import 'package:memolanes/common/map_fog_style.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:uuid/uuid.dart';

const _strings = 'general.map_settings.basemaps';

String mapStyleLabel(BuildContext context, MapStyle style) =>
    style.customName ??
    context.tr('general.map_settings.style_name.${style.id}');

class MapStylesPage extends StatefulWidget {
  const MapStylesPage({super.key, this.store});

  final MapStyleStore? store;

  @override
  State<MapStylesPage> createState() => _MapStylesPageState();
}

class _MapStylesPageState extends State<MapStylesPage> {
  MapStyleStore get _store => widget.store ?? MapStyleStore.instance;
  String? _error;

  @override
  void initState() {
    super.initState();
    _store.addListener(_refresh);
  }

  void _refresh() => setState(() {});

  @override
  void dispose() {
    _store.removeListener(_refresh);
    super.dispose();
  }

  void _select(MapStyle style) {
    try {
      _store.select(style);
      setState(() => _error = null);
    } on StateError {
      setState(() => _error = context.tr('$_strings.write_failed'));
    }
  }

  Future<void> _edit([MapStyle? style]) async {
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) => _MapStyleEditorPage(store: _store, initial: style),
      ),
    );
    if (mounted) setState(() => _error = null);
  }

  Widget _heading(String title) => Padding(
    padding: const EdgeInsets.fromLTRB(8, 12, 8, 8),
    child: Text(
      title,
      style: AppTypography.sectionLabel.copyWith(
        color: context.appColors.deepGreen,
      ),
    ),
  );

  Widget _option(MapStyle style, String selectedId) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: AppOptionTile(
        icon: style.type == MapStyleType.raster
            ? Icons.layers_outlined
            : Icons.map_outlined,
        title: mapStyleLabel(context, style),
        subtitle: style.isCustom
            ? context.tr('$_strings.type_${style.type.name}')
            : null,
        selected: style.id == selectedId,
        trailing: AppOptionTileTrailing.selection,
        action: style.isCustom
            ? IconButton(
                tooltip: context.tr('$_strings.edit'),
                onPressed: () => _edit(style),
                icon: Icon(
                  Icons.edit_outlined,
                  size: 20,
                  color: context.appColors.mutedInkColor,
                ),
              )
            : null,
        onTap: () => _select(style),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final custom = _store.customStyles;
    final selectedId = _store.selected.id;
    return Scaffold(
      appBar: CapsuleStyleAppBar(title: context.tr('$_strings.title')),
      body: SettingsPageLayout(
        children: [
          _heading(context.tr('$_strings.built_in')),
          for (final style in MapStyle.all) _option(style, selectedId),
          _heading(context.tr('$_strings.my_maps')),
          if (custom.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16),
              child: Text(
                context.tr('$_strings.empty'),
                style: AppTypography.supporting.copyWith(
                  color: context.appColors.mutedInkColor,
                ),
              ),
            ),
          for (final style in custom) _option(style, selectedId),
          const SizedBox(height: 8),
          AppButton(
            label: context.tr('$_strings.add'),
            icon: Icons.add,
            expand: true,
            borderRadius: 24,
            variant: AppButtonVariant.secondary,
            onPressed: _edit,
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                _error!,
                style: AppTypography.supporting.copyWith(
                  color: context.appColors.dangerColor,
                ),
              ),
            ),
          const SizedBox(height: 8),
          Center(
            child: TextButton.icon(
              onPressed: () => showMapCopyrightDialog(context, store: _store),
              icon: const Icon(Icons.info_outline, size: 16),
              label: Text(context.tr('$_strings.copyright')),
            ),
          ),
        ],
      ),
    );
  }
}

class _MapStyleEditorPage extends StatefulWidget {
  const _MapStyleEditorPage({required this.store, this.initial});

  final MapStyleStore store;
  final MapStyle? initial;

  @override
  State<_MapStyleEditorPage> createState() => _MapStyleEditorPageState();
}

class _MapStyleEditorPageState extends State<_MapStyleEditorPage> {
  final _form = GlobalKey<FormState>();
  late final TextEditingController _name;
  late final TextEditingController _url;
  late final TextEditingController _copyright;
  late final String _id;
  late MapStyleType _type;
  late int _tileSize;
  late MapTileScheme _scheme;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final initial = widget.initial;
    _id = initial?.id ?? 'custom:${const Uuid().v4()}';
    _name = TextEditingController(text: initial?.customName ?? '');
    _url = TextEditingController(text: initial?.url ?? '');
    _copyright = TextEditingController(text: initial?.copyright ?? '');
    _type = initial?.type ?? MapStyleType.style;
    _tileSize = initial?.tileSize ?? 256;
    _scheme = initial?.scheme ?? MapTileScheme.xyz;
  }

  @override
  void dispose() {
    _name.dispose();
    _url.dispose();
    _copyright.dispose();
    super.dispose();
  }

  String? _validateUrl(String? value) {
    final text = value?.trim() ?? '';
    if (text.isEmpty) return context.tr('$_strings.required');
    final uri = Uri.tryParse(text);
    if (uri == null ||
        !['http', 'https'].contains(uri.scheme) ||
        uri.host.isEmpty) {
      return context.tr('$_strings.invalid_url');
    }
    if (_type == MapStyleType.raster &&
        !['{z}', '{x}', '{y}'].every(text.contains)) {
      return context.tr('$_strings.invalid_tiles');
    }
    return null;
  }

  void _save() {
    if (_saving || !_form.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    final style = MapStyle(
      id: _id,
      customName: _name.text.trim(),
      url: _url.text.trim(),
      copyright: _copyright.text.trim(),
      type: _type,
      tileSize: _tileSize,
      scheme: _scheme,
      fogOpacityByStyle: const {
        MapFogStyle.dark: 0.50,
        MapFogStyle.light: 0.50,
      },
    );
    try {
      widget.store.save(style, selectAfterSaving: widget.initial == null);
      Navigator.of(context).pop();
    } on StateError {
      setState(() {
        _saving = false;
        _error = context.tr('$_strings.write_failed');
      });
    }
  }

  Future<void> _delete() async {
    final confirmed = await showAppDialog<bool>(
      context,
      builder: (dialogContext) => AppDialogContent(
        title: context.tr('$_strings.delete_title'),
        actions: AppDialogActions(
          children: [
            AppButton(
              label: context.tr('$_strings.cancel'),
              variant: AppButtonVariant.secondary,
              onPressed: () => Navigator.of(dialogContext).pop(false),
            ),
            AppButton(
              label: context.tr('$_strings.delete'),
              variant: AppButtonVariant.danger,
              onPressed: () => Navigator.of(dialogContext).pop(true),
            ),
          ],
        ),
        child: Text(
          context.tr(
            widget.store.selected.id == _id
                ? '$_strings.delete_selected_message'
                : '$_strings.delete_message',
            namedArgs: {'name': widget.initial!.customName!},
          ),
        ),
      ),
    );
    if (!mounted || confirmed != true) return;
    try {
      widget.store.delete(_id);
      Navigator.of(context).pop();
    } on StateError {
      setState(() => _error = context.tr('$_strings.write_failed'));
    }
  }

  InputDecoration _decoration(String key, {String? hint, String? helper}) =>
      InputDecoration(
        labelText: context.tr('$_strings.$key'),
        hintText: hint,
        helperText: helper,
        helperMaxLines: 3,
        errorMaxLines: 3,
        alignLabelWithHint: true,
      );

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr(
          widget.initial == null ? '$_strings.add' : '$_strings.edit',
        ),
      ),
      body: Form(
        key: _form,
        child: SettingsPageLayout(
          children: [
            TextFormField(
              controller: _name,
              maxLength: 50,
              decoration: _decoration(
                'name',
                hint: context.tr('$_strings.name_hint'),
              ).copyWith(counterText: ''),
              validator: (value) => value == null || value.trim().isEmpty
                  ? context.tr('$_strings.required')
                  : null,
            ),
            const SizedBox(height: 20),
            SizedBox(
              width: double.infinity,
              child: SegmentedButton<MapStyleType>(
                segments: [
                  ButtonSegment(
                    value: MapStyleType.style,
                    label: Text(context.tr('$_strings.type_style')),
                  ),
                  ButtonSegment(
                    value: MapStyleType.raster,
                    label: Text(context.tr('$_strings.type_raster')),
                  ),
                ],
                selected: {_type},
                onSelectionChanged: (value) =>
                    setState(() => _type = value.single),
              ),
            ),
            const SizedBox(height: 20),
            TextFormField(
              controller: _url,
              minLines: 2,
              maxLines: 4,
              keyboardType: TextInputType.url,
              autocorrect: false,
              enableSuggestions: false,
              decoration: _decoration(
                _type == MapStyleType.raster ? 'tile_url' : 'style_url',
                hint: _type == MapStyleType.raster
                    ? 'https://tile.openstreetmap.org/{z}/{x}/{y}.png'
                    : 'https://…/style.json',
                helper: context.tr(
                  _type == MapStyleType.raster
                      ? '$_strings.tile_url_help'
                      : '$_strings.style_url_help',
                ),
              ),
              validator: _validateUrl,
            ),
            if (_type == MapStyleType.raster) ...[
              const SizedBox(height: 8),
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(context.tr('$_strings.tile_options')),
                subtitle: Text('$_tileSize px / ${_scheme.name.toUpperCase()}'),
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: DropdownButtonFormField<int>(
                          initialValue: _tileSize,
                          decoration: _decoration('tile_size'),
                          items: const [
                            DropdownMenuItem(value: 256, child: Text('256 px')),
                            DropdownMenuItem(value: 512, child: Text('512 px')),
                          ],
                          onChanged: (value) {
                            if (value != null) {
                              setState(() => _tileSize = value);
                            }
                          },
                        ),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: DropdownButtonFormField<MapTileScheme>(
                          initialValue: _scheme,
                          decoration: _decoration('scheme'),
                          items: [
                            for (final scheme in MapTileScheme.values)
                              DropdownMenuItem(
                                value: scheme,
                                child: Text(scheme.name.toUpperCase()),
                              ),
                          ],
                          onChanged: (value) {
                            if (value != null) setState(() => _scheme = value);
                          },
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                ],
              ),
            ],
            const SizedBox(height: 20),
            TextFormField(
              controller: _copyright,
              minLines: 2,
              maxLines: 5,
              decoration: _decoration(
                'attribution',
                helper: context.tr(
                  _type == MapStyleType.raster
                      ? '$_strings.attribution_raster_help'
                      : '$_strings.attribution_style_help',
                ),
              ),
            ),
            const SizedBox(height: 24),
            if (_error != null) ...[
              Text(
                _error!,
                style: AppTypography.supporting.copyWith(
                  color: context.appColors.dangerColor,
                ),
              ),
              const SizedBox(height: 12),
            ],
            AppButton(
              label: context.tr(
                widget.initial == null
                    ? '$_strings.save_and_use'
                    : '$_strings.save',
              ),
              icon: Icons.check,
              expand: true,
              borderRadius: 24,
              onPressed: _saving ? null : _save,
            ),
            if (widget.initial != null) ...[
              const SizedBox(height: 8),
              Center(
                child: TextButton.icon(
                  style: TextButton.styleFrom(
                    foregroundColor: context.appColors.dangerColor,
                  ),
                  onPressed: _saving ? null : _delete,
                  icon: const Icon(Icons.delete_outline),
                  label: Text(context.tr('$_strings.delete')),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
