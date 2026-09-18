import 'package:flutter_hbb/common/web_direct_default_id.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('saved last_remote_id wins over config and page host', () {
    expect(
      resolveWebDirectRemoteId(
        isWeb: true,
        direct: true,
        lastRemoteId: '10.0.0.9',
        defaultTarget: '192.168.1.50',
        locationHost: '192.168.1.10',
      ),
      '10.0.0.9',
    );
  });

  test('config defaultTarget wins over location host when last id is empty', () {
    expect(
      resolveWebDirectRemoteId(
        isWeb: true,
        direct: true,
        lastRemoteId: '',
        defaultTarget: '192.168.1.50',
        locationHost: '192.168.1.10',
      ),
      '192.168.1.50',
    );
  });

  test('empty first visit on a LAN IP prefills that IP', () {
    expect(
      resolveWebDirectRemoteId(
        isWeb: true,
        direct: true,
        lastRemoteId: '  ',
        defaultTarget: '',
        locationHost: '192.168.1.10',
      ),
      '192.168.1.10',
    );
  });

  test('localhost / ::1 prefill 127.0.0.1', () {
    expect(usefulDirectConnectHost('localhost'), '127.0.0.1');
    expect(usefulDirectConnectHost('LOCALHOST'), '127.0.0.1');
    expect(usefulDirectConnectHost('127.0.0.1'), '127.0.0.1');
    expect(usefulDirectConnectHost('::1'), '127.0.0.1');
    expect(usefulDirectConnectHost('[::1]'), '127.0.0.1');
  });

  test('does not prefill when not web, not direct, or hostname-only', () {
    expect(
      resolveWebDirectRemoteId(
        isWeb: false,
        direct: true,
        lastRemoteId: '',
        defaultTarget: '',
        locationHost: '192.168.1.10',
      ),
      '',
    );
    expect(
      resolveWebDirectRemoteId(
        isWeb: true,
        direct: false,
        lastRemoteId: '',
        defaultTarget: '192.168.1.50',
        locationHost: '192.168.1.10',
      ),
      '',
    );
    expect(usefulDirectConnectHost('my-nas.local'), isNull);
    expect(usefulDirectConnectHost('localhost.example'), isNull);
  });
}
