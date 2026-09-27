"""Independent source-derived semantic checks, usable on another builder's JSON."""
import json
from pathlib import Path


def verify(records):
    assert len(records) == 9
    assert {(r['component'], r['variant']) for r in records} == {
        ('coredns', 'host-dual'), ('coredns', 'container-dual'), ('coredns', 'host-ipv4'), ('coredns', 'container-ipv4'),
        ('localpath', 'local'), ('localpath', 'shared'), ('portainer', 'sync'), ('portainer', 'async'), ('d2k', 'custom-namespace')}

    for record in records:
        component, variant, objects = record['component'], record['variant'], record['objects']
        assert record['checks']['repeat_success'] is True
        assert record['checks']['injected_error_propagated'] is True
        keys = {'repeat_success', 'injected_error_propagated'}
        if component == 'portainer':
            keys |= {'bootstrap_existing_config_preserved', 'bootstrap_existing_secret_preserved'}
        if component == 'd2k':
            keys |= {'missing_tls_input_rejected', 'tls_secret_updated'}
        assert set(record['checks']) == keys
        assert all(value is True for value in record['checks'].values())
        deployment = objects['deployments'][0]
        assert len(objects['deployments']) == 1
        spec = deployment['spec']
        assert spec['replicas'] == 1
        pod = spec['template']['spec']
        labels = spec['template']['metadata']['labels']
        assert all(labels[k] == v for k, v in spec['selector']['matchLabels'].items())
        assert pod['serviceAccountName'] in {a['metadata']['name'] for a in objects['serviceaccounts']}
        for service in objects['services'] or []:
            assert all(labels[k] == v for k, v in service['spec']['selector'].items())
        for binding in (objects['rolebindings'] or []) + (objects['clusterrolebindings'] or []):
            for subject in binding['subjects']:
                if subject['kind'] == 'ServiceAccount':
                    assert (subject['name'], subject['namespace']) in {(a['metadata']['name'], a['metadata']['namespace']) for a in objects['serviceaccounts']}
        container = pod['containers'][0]
        if component == 'coredns':
            assert container['image'] == 'docker.io/coredns/coredns:1.14.4'
            service = objects['services'][0]['spec']
            assert service['clusterIP'] == '10.43.0.10'
            assert {(p['port'], p['protocol']) for p in service['ports']} == {(53, 'UDP'), (53, 'TCP')}
            corefile = objects['configmaps'][0]['data']['Corefile']
            assert 'kubernetes cluster.local' in corefile
            assert ('ip6.arpa' in corefile) == variant.endswith('dual')
            assert ('/etc/resolv.conf' in corefile) == variant.startswith('host')
            if variant.startswith('container'):
                assert '1.1.1.1 8.8.8.8' in corefile
                assert not container['resources'].get('limits')
            else:
                assert container['resources']['limits']['memory'] == '64Mi'
        elif component == 'localpath':
            assert container['image'] == 'docker.io/rancher/local-path-provisioner:v0.0.36'
            storage = objects['storageclasses'][0]
            assert storage['provisioner'] == 'rancher.io/local-path'
            assert storage['reclaimPolicy'] == 'Retain'
            assert storage['volumeBindingMode'] == 'WaitForFirstConsumer'
            assert storage['metadata']['annotations']['storageclass.kubernetes.io/is-default-class'] == 'true'
            data = objects['configmaps'][0]['data']
            config = json.loads(data['config.json'])
            expected = {'sharedFileSystemPath': '/fixture/shared'} if variant == 'shared' else {'nodePathMap': [{'node': 'DEFAULT_PATH_FOR_NON_LISTED_NODES', 'paths': ['/fixture/storage']}]}
            assert config == expected
            assert 'image: busybox' in data['helperPod.yaml']
        elif component == 'portainer':
            assert container['image'] == 'portainer/agent:fixture'
            data = objects['configmaps'][0]['data']
            assert data['EDGE_ASYNC'] == str(variant == 'async').lower()
            assert data['EDGE_ID'] == 'fixture-id'
            assert data['EDGE_SECRET'] == 'fixture-secret'
            assert objects['secrets'][0]['stringData']['edge.key'] == 'fixture-key'
            assert objects['services'][0]['spec']['clusterIP'] == 'None'
        elif component == 'd2k':
            assert deployment['metadata']['namespace'] == 'fixture-d2k'
            assert container['image'] == 'fixture/d2k:fixed'
            assert objects['services'][0]['spec']['ports'][0]['port'] == 2376
            env = {e['name']: e for e in container['env']}
            assert env['D2K_NAMESPACE']['valueFrom']['fieldRef']['fieldPath'] == 'metadata.namespace'
            assert env['D2K_PORT']['value'] == '2376'
            assert env['D2K_SWARM_MODE']['value'] == 'true'
            assert pod['volumes'][0]['secret']['secretName'] == 'd2k-tls'
            assert objects['secrets'][0]['type'] == 'kubernetes.io/tls'
            assert set(objects['secrets'][0]['data']) == {'tls.crt', 'tls.key'}
            assert container['volumeMounts'][0]['readOnly'] is True
        else:
            raise AssertionError('unexpected component')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    verify([r for name in ('coredns', 'localpath', 'portainer', 'd2k') for r in json.loads((args.directory / (name + '.json')).read_text())])
    print('9 resource variants passed independent semantic checks')
