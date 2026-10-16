import 'dart:typed_data';

import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/map_webview_resources.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/renderer/internal_server.dart';

void main() {
  test(
    'custom scheme forwards path, query and binary body unchanged',
    () async {
      final proxy = _Proxy();
      final result = await MapWebViewResources.loadApiResource(
        proxy,
        WebUri('memolanes://api/tiles/3/2/1.pbf?v=abc&label=a%20b'),
      );
      expect(proxy.path, 'tiles/3/2/1.pbf');
      expect(proxy.query, {'v': 'abc', 'label': 'a b'});
      expect(result.data, same(proxy.response.body));
      expect(result.statusCode, 200);
      expect(result.contentType, proxy.response.contentType);
      expect(result.contentEncoding, 'utf-8');
      expect(
        result.headers,
        containsPair('Content-Type', proxy.response.contentType),
      );
      expect(result.headers, containsPair('X-Tile-Version', '7'));
      expect(
        result.headers,
        containsPair(
          'Access-Control-Expose-Headers',
          'X-Tile-Version, X-Not-Modified',
        ),
      );
    },
  );

  for (final status in [200, 304]) {
    test(
      'Android preserves response status and cache headers: $status',
      () async {
        final proxy = _Proxy(status: status);
        final result = await MapWebViewResources.interceptApiRequest(
          proxy,
          WebUri('https://memolanes.local/api/tiles/3/2/1.pbf'),
        );
        expect(proxy.path, 'tiles/3/2/1.pbf');
        expect(result!.data, same(proxy.response.body));
        expect(result.statusCode, status);
        expect(result.reasonPhrase, status == 200 ? 'OK' : 'Not Modified');
        expect(result.headers, containsPair('X-Tile-Version', '7'));
        expect(result.headers, containsPair('X-Not-Modified', 'true'));
        expect(
          result.headers,
          containsPair('Access-Control-Allow-Origin', '*'),
        );
      },
    );
  }

  for (final url in [
    'https://appassets.androidplatform.net/assets/index.html',
    'https://memolanes.local/api-other/path',
    'https://example.org/api/path',
  ]) {
    test('ordinary resources skip the Rust dispatcher: $url', () async {
      final proxy = _Proxy();
      expect(
        await MapWebViewResources.interceptApiRequest(proxy, WebUri(url)),
        isNull,
      );
      expect(proxy.path, isNull);
    });
  }

  test('backend headers retain precedence over adapter defaults', () async {
    final proxy = _Proxy(
      headers: {
        'Content-Type': 'application/custom',
        'Access-Control-Allow-Origin': 'https://map.local',
      },
    );
    final result = await MapWebViewResources.loadApiResource(
      proxy,
      WebUri('memolanes://api/data'),
    );
    expect(result.headers, containsPair('Content-Type', 'application/custom'));
    expect(
      result.headers,
      containsPair('Access-Control-Allow-Origin', 'https://map.local'),
    );
  });
}

class _Proxy extends Fake implements api.MapRendererProxy {
  _Proxy({int status = 200, Map<String, String> headers = const {}})
    : response = WebviewResponse(
        status: status,
        contentType: 'application/x-protobuf',
        body: Uint8List.fromList([0, 255, 65]),
        headers: {'X-Tile-Version': '7', 'X-Not-Modified': 'true', ...headers},
      );
  final WebviewResponse response;
  String? path;
  Map<String, String>? query;

  @override
  Future<WebviewResponse> handleRequest({
    required String path,
    required Map<String, String> queryParams,
  }) async {
    this.path = path;
    query = queryParams;
    return response;
  }
}
