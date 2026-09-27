"""Source-derived config, API, concurrency and owned-socket expectations."""
import copy
import hashlib
import json
from pathlib import Path

DEFAULT = {
    'apiVersion': 'kubesolo.io/v1alpha1', 'kind': 'Config', 'path': '/var/lib/kubesolo',
    'logging': {'debug': False, 'pprof': False},
    'network': {'nodeIP': '', 'mtu': 0, 'disableIPv6': False, 'loadBalancer': {'enabled': True, 'ip': ''}},
    'runtime': {'endpoint': ''},
    'kubernetes': {'apiServer': {'startupTimeoutSeconds': 600}, 'kubelet': {'cpuManager': {'policy': 'none', 'reservedCPUs': ''}}},
    'storage': {'dbWALRepair': False, 'localPath': {'enabled': True, 'sharedPath': ''}},
    'portainer': {'async': False, 'edgeID': '', 'edgeKey': '', 'image': 'docker.io/portainer/agent:lts'},
    'd2k': {'enabled': False, 'namespace': 'd2k'},
    'metrics': {'enabled': False, 'bindAddress': '127.0.0.1:9105'},
    'api': {'enabled': False, 'socketPath': ''},
}
SETTINGS = set('api.enabled api.socketPath d2k.enabled d2k.namespace kubernetes.apiServer.extraSANs kubernetes.apiServer.startupTimeoutSeconds kubernetes.kubelet.cpuManager.policy kubernetes.kubelet.cpuManager.policyOptions kubernetes.kubelet.cpuManager.reservedCPUs kubernetes.kubelet.systemReserved kubernetes.nodeName logging.debug logging.pprof metrics.bindAddress metrics.enabled network.disableIPv6 network.loadBalancer.enabled network.loadBalancer.ip network.mtu network.nodeIP path portainer.async portainer.edgeID portainer.edgeKey portainer.image runtime.containerMode runtime.endpoint storage.dbWALRepair storage.localPath.enabled storage.localPath.sharedPath'.split())
STATUS = {name: 200 for name in ['get', 'show-secrets', 'schema', 'patch-current-etag', 'patch-null', 'validate', 'put-replacement', 'delete-defaults', 'after-concurrent-patches', 'health', 'no-op-patch']}
STATUS.update({'patch-stale-etag': 412, 'immutable': 409, 'invalid': 422, 'malformed-patch': 400, 'malformed-put': 400, 'wrong-type': 400, 'wrong-content': 415, 'redacted-write': 400, 'oversized': 400, 'validate-invalid': 422})
CHECKS = {'validate_does_not_write', 'rejected_requests_do_not_write', 'live_socket_refused', 'shutdown_removes_socket', 'stale_socket_reclaimed', 'stale_socket_removed', 'regular_file_refused', 'regular_file_preserved'}


def typed_equal(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(typed_equal(actual[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(typed_equal(a, b) for a, b in zip(actual, expected))
    return actual == expected


def verify(file_records, api_records):
    assert len(file_records) == len(api_records) == 1
    file, api = file_records[0], api_records[0]
    assert file['target_mode'] == '0600'
    assert file['ordinary_backup_mode'] == '0600'
    # Characterized backup weaknesses; no claim these modes are desired Rust policy.
    assert file['backup_mode'] == '0400'
    assert file['preexisting_backup_mode'] == '0644'
    assert file['remaining_entries'] == ['config.yaml', 'config.yaml.bak']
    for key in ('backup_matches_first', 'failed_write_rejected', 'failed_write_preserves_original'):
        assert file[key] is True
    assert 'mtu: 0\n' in file['first']
    assert file['second'] == file['first'].replace('mtu: 0\n', 'mtu: 1400\n')
    assert 'edgeKey: fixture-synthetic-key' in file['first']
    assert api['socket_mode'] == '0600'
    assert api['effective_node_ip'] == '192.0.2.99'
    assert set(api['checks']) == CHECKS
    assert all(value is True for value in api['checks'].values())
    requests = api['requests']
    assert set(requests) == set(STATUS)
    for name, request in requests.items():
        assert request['status'] == STATUS[name], name
        if name == 'health':
            assert request['body'] == request['raw_body'] == 'ok\n'
        else:
            assert typed_equal(json.loads(request['raw_body']), request['body'])
    current = copy.deepcopy(DEFAULT)
    current['network']['nodeIP'] = '192.0.2.10'
    current['d2k']['namespace'] = 'workloads'
    current['portainer']['edgeKey'] = '***'
    assert typed_equal(requests['get']['body'], {'config': current, 'restartRequired': False})
    revealed = copy.deepcopy(current)
    revealed['portainer']['edgeKey'] = 'fixture-synthetic-key'
    assert typed_equal(requests['show-secrets']['body']['config'], revealed)
    # Raw HTTP JSON retains Go's struct key order; independently hash the actual
    # unredacted config document rather than calling etagOf from the baseline.
    raw_config = json.loads(requests['show-secrets']['raw_body'])['config']
    tag = '"' + hashlib.sha256(json.dumps(raw_config, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest() + '"'
    assert requests['get']['etag'] == requests['show-secrets']['etag'] == tag
    current['network']['mtu'] = 1400
    assert typed_equal(requests['patch-current-etag']['body']['config'], current)
    assert requests['patch-current-etag']['etag'] != tag
    body = requests['patch-current-etag']['body']
    assert body['changed'] == body['requiresRestart'] == ['network.mtu']
    assert body['restartRequired'] is True
    current['d2k']['namespace'] = 'd2k'
    assert typed_equal(requests['patch-null']['body']['config'], current)
    assert requests['immutable']['body']['field'] == 'path'
    assert 'portainer.edgeKey' in requests['redacted-write']['body']['error']
    assert 'request body too large' in requests['oversized']['body']['error']
    replaced = copy.deepcopy(DEFAULT)
    replaced['network']['mtu'] = 1450
    assert typed_equal(requests['put-replacement']['body']['config'], replaced)
    assert typed_equal(requests['delete-defaults']['body']['config'], DEFAULT)
    concurrent = copy.deepcopy(DEFAULT)
    concurrent['network']['mtu'] = 1400
    concurrent['logging']['debug'] = True
    assert typed_equal(requests['after-concurrent-patches']['body']['config'], concurrent)
    assert typed_equal(requests['no-op-patch']['body'], {'config': concurrent, 'restartRequired': False})
    settings = requests['schema']['body']['settings']
    assert len(settings) == 30 and {s['path'] for s in settings} == SETTINGS
    assert next(s for s in settings if s['path'] == 'path')['mutability'] == 'immutable'
    assert next(s for s in settings if s['path'] == 'portainer.edgeKey')['secret'] is True


if __name__ == '__main__':
    import sys
    directory = Path(sys.argv[1])
    verify(json.loads((directory / 'config.json').read_text()), json.loads((directory / 'configapi.json').read_text()))
    print('Atomic configuration and 21 real Unix HTTP scenarios passed')
