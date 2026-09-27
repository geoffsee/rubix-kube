"""Independent certificate policy and trust-preserving restart expectations."""
import json
from pathlib import Path

SUBJECTS = {
    'ca': ('kubernetes-ca', ['Kubernetes'], True, 3650),
    'kubelet': ('system:node:fixture-node', ['system:nodes'], False, 365),
    'apiserver': ('kube-apiserver', ['Kubernetes'], False, 365),
    'controller-manager': ('system:kube-controller-manager', ['system:kube-controller-manager'], False, 365),
    'admin': ('kubesolo-admin', ['system:masters'], False, 365),
    'webhook': ('kubesolo-webhook', ['system:masters'], False, 365),
    'request-header-ca': ('request-header-ca', ['Kubernetes'], True, 3650),
    'request-header-client': ('system:auth-proxy', ['system:auth-proxy'], False, 365),
    'd2k-server': ('d2k', ['kubesolo'], False, 365),
    'd2k-client': ('d2k-client', ['kubesolo'], False, 365),
}

# Source: defaultCertOptions/configureCertificateByType. Arrays preserve source
# ordering and duplicate SANs (D2K explicitly prepends loopback to local IPs).
PUBLIC = {
    'ca': (None, [], None, 97),
    'request-header-ca': (None, [], None, 97),
    'admin': (['localhost'], ['127.0.0.1'], [2, 1], 5),
    'kubelet': (['fixture-node', 'localhost'], ['127.0.0.1'], [2, 1], 5),
    'controller-manager': (None, [], [2], 5),
    'request-header-client': (None, [], [2], 5),
    'd2k-client': (None, [], [2], 5),
    'webhook': (['localhost', 'kubesolo-webhook', 'kubesolo-webhook.default', 'kubesolo-webhook.default.svc'], ['127.0.0.1'], [1], 5),
    'd2k-server': (['d2k', 'd2k.fixture-d2k', 'd2k.fixture-d2k.svc', 'd2k.fixture-d2k.svc.cluster.local', 'localhost'], ['127.0.0.1', '127.0.0.1'], [1], 5),
}
API_DNS = ['kubernetes', 'kubernetes.default', 'kubernetes.default.svc', 'kubernetes.default.svc.cluster', 'kubernetes.default.svc.cluster.local', 'localhost', 'api.fixture.test']


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(records):
    require(len(records) == 1, 'PKI expectation failed: len(records) == 1')
    record = records[0]
    require(record['restart_unchanged'] is True, "PKI expectation failed: record['restart_unchanged'] is True")
    require(record['invalid_extra_sans_ignored'] is True, "PKI expectation failed: record['invalid_extra_sans_ignored'] is True")
    require(record['ipv6_extra_san_rotates'] is False, 'baseline IPv6 SAN rotation changed')
    # Characterization of a baseline weakness, not desired repair behavior.
    require(record['existing_corrupt_key_is_skipped'] is True, "PKI expectation failed: record['existing_corrupt_key_is_skipped'] is True")
    require(record['existing_mismatched_key_is_skipped'] is True, "PKI expectation failed: record['existing_mismatched_key_is_skipped'] is True")
    require(record['unsafe_roots_rejected'] == {'': True, '/': True, '.': True, 'symlink': True}, "PKI expectation failed: record['unsafe_roots_rejected'] == {'': True, '/': True, '.': True, 'symlink': True}")
    require(set(record['rotations']) == {'node-ip-change', 'extra-san-change', 'corrupt-leaf', 'expired-leaf'}, "PKI expectation failed: set(record['rotations']) == {'node-ip-change', 'extra-san-change', 'corrupt-leaf', 'expired-leaf'}")
    for scenario, certificates in [('fresh', record['fresh'])] + [(k, v['certificates']) for k, v in record['rotations'].items()]:
        require(set(certificates) == set(SUBJECTS), 'PKI expectation failed: set(certificates) == set(SUBJECTS)')
        for name, cert in certificates.items():
            require(type(cert['is_ca']) is bool, 'is_ca must be a boolean')
            require(all(type(cert[field]) is int for field in ('valid_days', 'key_bits', 'key_usage')), 'certificate numeric fields must be integers')
            require(cert['extended_key_usage'] is None or all(type(value) is int for value in cert['extended_key_usage']), 'extended usages must be integers')
            cn, orgs, ca, days = SUBJECTS[name]
            require((cert['cn'], cert['organizations'], cert['is_ca'], cert['valid_days']) == (cn, orgs, ca, days), "PKI expectation failed: (cert['cn'], cert['organizations'], cert['is_ca'], cert['valid_days']) == (cn, orgs, ca, days)")
            if name == 'apiserver':
                names = API_DNS + ([] if scenario in ('fresh', 'node-ip-change') else ['new.fixture.test'])
                addresses = ['10.43.0.1', '127.0.0.1', '192.0.2.10' if scenario == 'fresh' else '192.0.2.11', '192.0.2.20']
                public = (names, addresses, [1, 2], 5)
            else:
                public = PUBLIC[name]
            require((cert['dns'], cert['ips'], cert['extended_key_usage'], cert['key_usage']) == public, "PKI expectation failed: (cert['dns'], cert['ips'], cert['extended_key_usage'], cert['key_usage']) == public")
            require(cert['key_bits'] == 2048, "PKI expectation failed: cert['key_bits'] == 2048")
            require(cert['key_mode'] == '0600', "PKI expectation failed: cert['key_mode'] == '0600'")
            require(all(cert[k] is True for k in ('chain_verified', 'key_matches', 'serial_positive', 'serial_bits_at_most_128')), "PKI expectation failed: all(cert[k] is True for k in ('chain_verified', 'key_matches', 'serial_positive', 'serial_bits_at_most_128'))")
        api = certificates['apiserver']
        require(set(api['ips']) == {'10.43.0.1', '127.0.0.1', '192.0.2.20', '192.0.2.10' if scenario == 'fresh' else '192.0.2.11'}, "PKI expectation failed: set(api['ips']) == {'10.43.0.1', '127.0.0.1', '192.0.2.20', '192.0.2.10' if scenario == 'fresh' else '192.0.2.11'}")
        require('api.fixture.test' in api['dns'], "PKI expectation failed: 'api.fixture.test' in api['dns']")
        require(('new.fixture.test' in api['dns']) == (scenario not in ('fresh', 'node-ip-change')), "PKI expectation failed: ('new.fixture.test' in api['dns']) == (scenario not in ('fresh', 'node-ip-change'))")
        require(set(certificates['d2k-server']['dns']) == {'d2k', 'd2k.fixture-d2k', 'd2k.fixture-d2k.svc', 'd2k.fixture-d2k.svc.cluster.local', 'localhost'}, "PKI expectation failed: set(certificates['d2k-server']['dns']) == {'d2k', 'd2k.fixture-d2k', 'd2k.fixture-d2k.svc', 'd2k.fixture-d2k.svc.cluster.local', 'localhost'}")
        require(certificates['ca']['key_usage'] & 32, "PKI expectation failed: certificates['ca']['key_usage'] & 32")
        require(certificates['request-header-ca']['key_usage'] & 32, "PKI expectation failed: certificates['request-header-ca']['key_usage'] & 32")
        for name in ('controller-manager', 'request-header-client', 'd2k-client'):
            require(certificates[name]['extended_key_usage'] == [2], "PKI expectation failed: certificates[name]['extended_key_usage'] == [2]")
        require(certificates['d2k-server']['extended_key_usage'] == [1], "PKI expectation failed: certificates['d2k-server']['extended_key_usage'] == [1]")
    for rotation in record['rotations'].values():
        require(rotation['stable_certificates'] == {name: name in ('ca', 'request-header-ca', 'request-header-client') for name in SUBJECTS}, "PKI expectation failed: rotation['stable_certificates'] == {name: name in ('ca', 'request-header-ca', 'request-header-client') for name in SUBJECTS}")


if __name__ == '__main__':
    import sys
    verify(json.loads(Path(sys.argv[1]).read_text()))
    print('PKI crypto metadata and four trust-preserving rotations passed')
