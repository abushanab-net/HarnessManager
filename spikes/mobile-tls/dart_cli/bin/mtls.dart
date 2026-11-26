// dart:io mutual TLS against the spike host (rustls configured like listener.rs): an Ed25519 self-signed client
// certificate, and the host's self-signed Ed25519 certificate pinned by exact DER. Three cases.
import 'dart:convert';
import 'dart:io';

final dir = 'certs';

Future<String> hello(String key, String crt, List<int> pin, {int port = 47802}) async {
  final ctx = SecurityContext(withTrustedRoots: false)
    ..useCertificateChainBytes(File('$dir/$crt').readAsBytesSync())
    ..usePrivateKeyBytes(File('$dir/$key').readAsBytesSync());
  final c = HttpClient(context: ctx)
    // called because the self-signed certificate does not chain to a root; accept only the pinned DER
    ..badCertificateCallback = (cert, host, port) => _same(cert.der, pin);
  try {
    final sw = Stopwatch()..start();
    final r = await (await c.getUrl(Uri.parse('https://127.0.0.1:$port/c1/hello'))).close();
    final body = await r.transform(utf8.decoder).join();
    return 'HTTP ${r.statusCode} in ${sw.elapsedMilliseconds} ms: $body';
  } catch (e) {
    return 'FAILED: $e';
  } finally {
    c.close(force: true);
  }
}

bool _same(List<int> a, List<int> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

Future<void> main() async {
  final pin = File('$dir/host.der').readAsBytesSync();
  final wrong = List<int>.of(pin)..[pin.length - 1] ^= 1;
  print('paired client, right pin : ${await hello('client.key', 'client.crt', pin)}');
  print('stranger client          : ${await hello('stranger.key', 'stranger.crt', pin)}');
  print('paired client, wrong pin : ${await hello('client.key', 'client.crt', wrong)}');
  // the same listener with an ECDSA P-256 server certificate; the client certificate is still Ed25519
  final p256 = File('../../mobile-host/host-data/host-p256.der').readAsBytesSync();
  print('P-256 server, right pin  : ${await hello('client.key', 'client.crt', p256, port: 47803)}');
  print('P-256 server, stranger   : ${await hello('stranger.key', 'stranger.crt', p256, port: 47803)}');
  print('P-256 server, wrong pin  : ${await hello('client.key', 'client.crt', pin, port: 47803)}');
}
