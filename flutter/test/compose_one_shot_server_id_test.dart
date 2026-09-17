import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/one_shot_server_id.dart';

void main() {
  test('composeOneShotServerId builds id@host?key= without clobbering an existing host',
      () {
    expect(
      composeOneShotServerId('1234', 'hbbs.example.com', 'abc+def='),
      '1234@hbbs.example.com?key=abc+def=',
    );
    expect(
      composeOneShotServerId('1234@other:21116', 'ignored.example.com', 'abc'),
      '1234@other:21116?key=abc',
    );
    expect(composeOneShotServerId('1234', null, 'abc'), '1234');
    expect(composeOneShotServerId('1234', '', 'abc'), '1234');
    expect(composeOneShotServerId('1234', null, null), '1234');
    expect(
      composeOneShotServerId('1234@hbbs.example.com?key=keep', 'other', 'new'),
      '1234@hbbs.example.com?key=keep',
    );
  });
}
