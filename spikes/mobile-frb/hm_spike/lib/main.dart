// Spike app: pairs with the spike host using the desktop's exact pairing messages (crypto in Rust through
// flutter_rust_bridge, HTTP in Dart), then reaches the companion API over mutual TLS (in Rust; dart:io is tried
// too and fails), then answers asks opened from a notification, reporting each step to the host for timing.
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hm_spike/src/rust/api/pairing.dart';
import 'package:hm_spike/src/rust/api/tls.dart';
import 'package:hm_spike/src/rust/frb_generated.dart';

const host = String.fromEnvironment('HOST', defaultValue: '10.0.2.2');
const code = String.fromEnvironment('CODE', defaultValue: '482913');
const ch = MethodChannel('hm/ask');
final pairBase = 'http://$host:47801';
final tlsBase = 'https://$host:47802';

Future<void> main() async {
  final sw = Stopwatch()..start();
  await RustLib.init();
  final rustInitMs = sw.elapsedMilliseconds;
  runApp(SpikeApp(rustInitMs: rustInitMs));
}

/// Plain HTTP to the pairing port, as `pairing::join` does.
Future<Map<String, dynamic>> _json(String method, String path, [Map<String, dynamic>? body]) async {
  final c = HttpClient()..connectionTimeout = const Duration(seconds: 5);
  try {
    final rq = await c.openUrl(method, Uri.parse('$pairBase$path'));
    if (body != null) {
      rq.headers.contentType = ContentType.json;
      rq.write(jsonEncode(body));
    }
    final r = await rq.close();
    final text = await r.transform(utf8.decoder).join();
    return {'_status': r.statusCode, ...(text.isEmpty ? {} : jsonDecode(text) as Map<String, dynamic>)};
  } finally {
    c.close();
  }
}

/// A step for the driver's log, on the host's clock. Not awaited by the measured path.
void mark(String event, Map<String, dynamic> extra) => _json('POST', '/spike/mark', {'event': 'app-$event', ...extra}).ignore();

class SpikeApp extends StatefulWidget {
  const SpikeApp({super.key, required this.rustInitMs});
  final int rustInitMs;
  @override
  State<SpikeApp> createState() => _SpikeAppState();
}

class _SpikeAppState extends State<SpikeApp> {
  final lines = <String>[];
  File? store;
  Map<String, dynamic>? paired; // {me, seed, cert_der, host_pem}
  C1? c1;
  Map<String, dynamic>? ask;
  String? receipt;

  void say(String s) {
    debugPrint('SPIKE $s');
    setState(() => lines.add(s));
  }

  @override
  void initState() {
    super.initState();
    _boot();
  }

  Future<void> _boot() async {
    say('Rust library loaded and bridge ready in ${widget.rustInitMs} ms');
    store = File('${await ch.invokeMethod<String>('files')}/paired.json');
    if (store!.existsSync()) paired = jsonDecode(store!.readAsStringSync());
    ch.setMethodCallHandler((call) async {
      if (call.method == 'ask') await _answer(Map<String, dynamic>.from(call.arguments as Map));
    });
    final initial = await ch.invokeMethod<Map>('initial');
    if (initial != null) return _answer(Map<String, dynamic>.from(initial));
    if (paired == null) {
      await _pair();
      await _tls();
    } else {
      say('Paired already as ${paired!['me']}');
    }
  }

  // ---- spike 1: the desktop's pairing exchange ----
  Future<Map<String, dynamic>> _exchange(String me, PhoneIdentity id, String code) async {
    final hello = await _json('GET', '/pair/hello');
    final (st, msg) = SpakeA.start(code: code, me: me, them: hello['id']);
    final r = await _json('POST', '/pair/start', {'id': me, 'msg': msg});
    final key = st.finish(inbound: r['msg']);
    final mine = {'id': me, 'name': 'Spike phone', 'os': 'android', 'platform': 'android', 'pubkey': id.pubkey, 'cert_pem': id.certPem, 'address': ''};
    final f = await _json('POST', '/pair/finish', {'id': me, 'payload': seal(key: key, json: jsonEncode(mine)), 'address': ''});
    if (f['_status'] != 200) return {'refused': f['_status'], 'error': f['error']};
    return jsonDecode(open(key: key, sealed: f['payload']));
  }

  Future<void> _pair() async {
    try {
      final me = 'phone-${Random.secure().nextInt(1 << 32).toRadixString(16)}';
      final sw = Stopwatch()..start();
      final id = phoneIdentity(seed: [], deviceId: me);
      final idMs = sw.elapsedMilliseconds;
      say('Identity: Ed25519 key + rcgen certificate (${id.certDer.length} B DER) in $idMs ms');
      final wrong = await _exchange(me, id, code == '000000' ? '111111' : '000000');
      say('Wrong code: host answered ${wrong['refused']} "${wrong['error']}"');
      sw.reset();
      final theirs = await _exchange(me, id, code);
      final pairMs = sw.elapsedMilliseconds;
      say('Paired with ${theirs['name']} (${theirs['id']}) in $pairMs ms; its certificate pinned, ring sent: ${theirs.containsKey('ring')}');
      paired = {'me': me, 'seed': base64.encode(id.seed), 'cert_der': base64.encode(id.certDer), 'key_pem': id.keyPem, 'cert_pem': id.certPem, 'host_pem': theirs['cert_pem']};
      store!.writeAsStringSync(jsonEncode(paired));
      mark('pair-result', {'ok': true, 'identity_ms': idMs, 'pair_ms': pairMs, 'wrong_code_status': wrong['refused'], 'host_name': theirs['name']});
    } catch (e) {
      say('PAIRING FAILED: $e');
      mark('pair-result', {'ok': false, 'error': '$e'});
    }
  }

  C1 _client() => c1 ??= C1.connect(
      seed: base64.decode(paired!['seed']), certDer: base64.decode(paired!['cert_der']), pinPem: paired!['host_pem'], base: tlsBase);

  // ---- spike 2: mutual TLS, Rust and dart:io ----
  Future<void> _tls() async {
    final res = <String, dynamic>{};
    try {
      final sw = Stopwatch()..start();
      final a = await _client().get_(path: '/c1/hello');
      final b = await _client().get_(path: '/c1/hello');
      res.addAll({'rust_status': a.status, 'rust_first_ms': sw.elapsedMilliseconds - b.ms, 'rust_reused_ms': b.ms});
      say('Rust mTLS /c1/hello: ${a.status} first ${a.ms} ms, reused ${b.ms} ms: ${a.body}');
    } catch (e) {
      res['rust_error'] = '$e';
      say('Rust mTLS FAILED: $e');
    }
    final ctx = SecurityContext(withTrustedRoots: false)
      ..useCertificateChainBytes(utf8.encode(paired!['cert_pem']))
      ..usePrivateKeyBytes(utf8.encode(paired!['key_pem']));
    final pin = base64.decode((paired!['host_pem'] as String).split(RegExp(r'\r?\n')).where((l) => !l.startsWith('-----')).join().trim());
    final c = HttpClient(context: ctx)..badCertificateCallback = (cert, h, p) => _same(cert.der, pin);
    try {
      final r = await (await c.getUrl(Uri.parse('$tlsBase/c1/hello'))).close();
      res['dart_status'] = r.statusCode;
      say('dart:io mTLS /c1/hello: ${r.statusCode}');
    } catch (e) {
      res['dart_error'] = '$e'.replaceAll('\n', ' ');
      say('dart:io mTLS failed: ${res['dart_error']}');
    } finally {
      c.close(force: true);
    }
    mark('tls-result', res);
  }

  // ---- spike 3: from the notification tap to an accepted answer ----
  Future<void> _answer(Map<String, dynamic> a) async {
    final sw = Stopwatch()..start();
    final up = await ch.invokeMethod<int>('uptime');
    final intentToDart = up! - (a['tapped_uptime'] as int);
    final hadClient = c1 != null;
    if (paired == null) return say('Not paired: cannot answer');
    final r = await _client().get_(path: '/c1/requests');
    final fetchMs = sw.elapsedMilliseconds;
    final req = (jsonDecode(r.body)['requests'] as List).cast<Map<String, dynamic>>().where((x) => x['ask_id'] == a['ask']).firstOrNull;
    setState(() {
      ask = req;
      receipt = null;
    });
    await WidgetsBinding.instance.endOfFrame;
    final shownMs = sw.elapsedMilliseconds;
    mark('shown', {'ask_id': a['ask'], 'cold': a['cold'], 'reused_client': hadClient, 'intent_to_dart_ms': intentToDart, 'fetch_ms': fetchMs, 'shown_ms': shownMs});
    if (req == null) return say('Ask ${a['ask']} is not open any more');
    // the human's Allow tap is not simulated: the answer goes as soon as the request is on screen
    final body = {'action_id': 'act-${a['ask']}', 'kind': 'answer', 'target': {'run': req['run'], 'ask_id': req['ask_id']},
      'state_version': req['state_version'], 'expires_at': req['expires_at'], 'payload': {'behavior': 'allow'}};
    final rc = await _client().post(path: '/c1/actions', json: jsonEncode(body));
    final doneMs = sw.elapsedMilliseconds;
    setState(() => receipt = rc.body);
    mark('answered', {'ask_id': a['ask'], 'status': jsonDecode(rc.body)['status'], 'done_ms': doneMs, 'action_ms': rc.ms});
    say('Ask ${a['ask']}: fetched $fetchMs ms, shown $shownMs ms, accepted $doneMs ms (${a['cold'] == true ? 'cold' : 'warm'})');
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
        home: Scaffold(
          appBar: AppBar(title: const Text('HM mobile spike')),
          body: ListView(padding: const EdgeInsets.all(12), children: [
            if (ask != null)
              Card(child: ListTile(title: Text('${ask!['tool']}: ${ask!['what']}'), subtitle: Text(receipt ?? 'answering…'))),
            for (final l in lines) Padding(padding: const EdgeInsets.symmetric(vertical: 4), child: Text(l)),
            Wrap(spacing: 8, children: [
              FilledButton(onPressed: () async { await _pair(); await _tls(); }, child: const Text('Pair again')),
              OutlinedButton(onPressed: paired == null ? null : _tls, child: const Text('TLS check')),
            ]),
          ]),
        ),
      );
}

bool _same(List<int> a, List<int> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}
