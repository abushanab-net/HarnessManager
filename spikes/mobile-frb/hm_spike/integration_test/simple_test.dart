import 'package:flutter_test/flutter_test.dart';
import 'package:hm_spike/src/rust/api/pairing.dart';
import 'package:hm_spike/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

// On the device: the Rust crypto answers through the bridge (the host interop run is in the app itself).
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  test('identity, SPAKE2 start and sealing cross the bridge', () {
    final id = phoneIdentity(seed: [], deviceId: 'phone-test');
    expect(id.seed.length, 32);
    expect(phoneIdentity(seed: id.seed, deviceId: 'phone-test').certDer, id.certDer); // deterministic, as identity.rs
    final (st, msg) = SpakeA.start(code: '123456', me: 'phone-test', them: 'host');
    expect(msg.isNotEmpty, true);
    expect(() => st.finish(inbound: 'AAAA'), throwsA(anything));
    final key = List<int>.generate(32, (i) => i);
    expect(open(key: key, sealed: seal(key: key, json: '{"a":1}')), '{"a":1}');
    expect(() => open(key: List<int>.filled(32, 7), sealed: seal(key: key, json: '{}')), throwsA(anything));
  });
}
