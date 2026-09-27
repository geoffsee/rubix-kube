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


def verify(records):
    assert len(records) == 1
    record = records[0]
    assert record['restart_unchanged'] is True
    assert record['invalid_extra_sans_ignored'] is True
    # Characterization of a baseline weakness, not desired repair behavior.
    assert record['existing_corrupt_key_is_skipped'] is True
    assert record['existing_mismatched_key_is_skipped'] is True
    assert record['unsafe_roots_rejected'] == {'': True, '/': True, '.': True, 'symlink': True}
    assert set(record['rotations']) == {'node-ip-change', 'extra-san-change', 'corrupt-leaf', 'expired-leaf'}
    for scenario, certificates in [('fresh', record['fresh'])] + [(k, v['certificates']) for k, v in record['rotations'].items()]:
        assert set(certificates) == set(SUBJECTS)
        for name, cert in certificates.items():
            cn, orgs, ca, days = SUBJECTS[name]
            assert (cert['cn'], cert['organizations'], cert['is_ca'], cert['valid_days']) == (cn, orgs, ca, days)
            if name == 'apiserver':
                names = API_DNS + ([] if scenario in ('fresh', 'node-ip-change') else ['new.fixture.test'])
                addresses = ['10.43.0.1', '127.0.0.1', '192.0.2.10' if scenario == 'fresh' else '192.0.2.11', '192.0.2.20']
                public = (names, addresses, [1, 2], 5)
            else:
                public = PUBLIC[name]
            assert (cert['dns'], cert['ips'], cert['extended_key_usage'], cert['key_usage']) == public
            assert cert['key_bits'] == 2048
            assert cert['key_mode'] == '0600'
            assert all(cert[k] is True for k in ('chain_verified', 'key_matches', 'serial_positive', 'serial_bits_at_most_128'))
        api = certificates['apiserver']
        assert set(api['ips']) == {'10.43.0.1', '127.0.0.1', '192.0.2.20', '192.0.2.10' if scenario == 'fresh' else '192.0.2.11'}
        assert 'api.fixture.test' in api['dns']
        assert ('new.fixture.test' in api['dns']) == (scenario not in ('fresh', 'node-ip-change'))
        assert set(certificates['d2k-server']['dns']) == {'d2k', 'd2k.fixture-d2k', 'd2k.fixture-d2k.svc', 'd2k.fixture-d2k.svc.cluster.local', 'localhost'}
        assert certificates['ca']['key_usage'] & 32
        assert certificates['request-header-ca']['key_usage'] & 32
        for name in ('controller-manager', 'request-header-client', 'd2k-client'):
            assert certificates[name]['extended_key_usage'] == [2]
        assert certificates['d2k-server']['extended_key_usage'] == [1]
    for rotation in record['rotations'].values():
        assert rotation['stable_certificates'] == {name: name in ('ca', 'request-header-ca', 'request-header-client') for name in SUBJECTS}


if __name__ == '__main__':
    import sys
    verify(json.loads(Path(sys.argv[1]).read_text()))
    print('PKI crypto metadata and four trust-preserving rotations passed')
