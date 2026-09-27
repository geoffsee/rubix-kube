"""Independent source-derived semantic checks, usable on another builder's JSON."""
import json
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(records):
    require(len(records) == 9, 'Resource expectation failed: len(records) == 9')
    require({(r['component'], r['variant']) for r in records} == {('coredns', 'host-dual'), ('coredns', 'container-dual'), ('coredns', 'host-ipv4'), ('coredns', 'container-ipv4'), ('localpath', 'local'), ('localpath', 'shared'), ('portainer', 'sync'), ('portainer', 'async'), ('d2k', 'custom-namespace')}, "Resource expectation failed: {(r['component'], r['variant']) for r in records} == {('coredns', 'host-dual'), ('coredns', 'container-dual'), ('coredns', 'host-ipv4'), ('coredns', 'container-ipv4'), ('localpath', 'local'), ('localpath', 'shared'), ('portainer', 'sync'), ('portainer', 'async'), ('d2k', 'custom-namespace')}")

    for record in records:
        component, variant, objects = record['component'], record['variant'], record['objects']
        require(record['checks']['repeat_success'] is True, "Resource expectation failed: record['checks']['repeat_success'] is True")
        require(record['checks']['injected_error_propagated'] is True, "Resource expectation failed: record['checks']['injected_error_propagated'] is True")
        keys = {'repeat_success', 'injected_error_propagated'}
        if component == 'portainer':
            keys |= {'bootstrap_existing_config_preserved', 'bootstrap_existing_secret_preserved'}
        if component == 'd2k':
            keys |= {'missing_tls_input_rejected', 'tls_secret_updated'}
        require(set(record['checks']) == keys, "Resource expectation failed: set(record['checks']) == keys")
        require(all((value is True for value in record['checks'].values())), "Resource expectation failed: all((value is True for value in record['checks'].values()))")
        deployment = objects['deployments'][0]
        require(len(objects['deployments']) == 1, "Resource expectation failed: len(objects['deployments']) == 1")
        spec = deployment['spec']
        require(type(spec['replicas']) is int, 'replicas must be an integer')
        require(spec['replicas'] == 1, "Resource expectation failed: spec['replicas'] == 1")
        pod = spec['template']['spec']
        labels = spec['template']['metadata']['labels']
        require(all((labels[k] == v for k, v in spec['selector']['matchLabels'].items())), "Resource expectation failed: all((labels[k] == v for k, v in spec['selector']['matchLabels'].items()))")
        require(pod['serviceAccountName'] in {a['metadata']['name'] for a in objects['serviceaccounts']}, "Resource expectation failed: pod['serviceAccountName'] in {a['metadata']['name'] for a in objects['serviceaccounts']}")
        for service in objects['services'] or []:
            require(all(type(port['port']) is int for port in service['spec']['ports']), 'service ports must be integers')
            require(all((labels[k] == v for k, v in service['spec']['selector'].items())), "Resource expectation failed: all((labels[k] == v for k, v in service['spec']['selector'].items()))")
        for binding in (objects['rolebindings'] or []) + (objects['clusterrolebindings'] or []):
            for subject in binding['subjects']:
                if subject['kind'] == 'ServiceAccount':
                    require((subject['name'], subject['namespace']) in {(a['metadata']['name'], a['metadata']['namespace']) for a in objects['serviceaccounts']}, "Resource expectation failed: (subject['name'], subject['namespace']) in {(a['metadata']['name'], a['metadata']['namespace']) for a in objects['serviceaccounts']}")
        container = pod['containers'][0]
        if component == 'coredns':
            require(container['image'] == 'docker.io/coredns/coredns:1.14.4', "Resource expectation failed: container['image'] == 'docker.io/coredns/coredns:1.14.4'")
            service = objects['services'][0]['spec']
            require(service['clusterIP'] == '10.43.0.10', "Resource expectation failed: service['clusterIP'] == '10.43.0.10'")
            require({(p['port'], p['protocol']) for p in service['ports']} == {(53, 'UDP'), (53, 'TCP')}, "Resource expectation failed: {(p['port'], p['protocol']) for p in service['ports']} == {(53, 'UDP'), (53, 'TCP')}")
            corefile = objects['configmaps'][0]['data']['Corefile']
            require('kubernetes cluster.local' in corefile, "Resource expectation failed: 'kubernetes cluster.local' in corefile")
            require(('ip6.arpa' in corefile) == variant.endswith('dual'), "Resource expectation failed: ('ip6.arpa' in corefile) == variant.endswith('dual')")
            require(('/etc/resolv.conf' in corefile) == variant.startswith('host'), "Resource expectation failed: ('/etc/resolv.conf' in corefile) == variant.startswith('host')")
            if variant.startswith('container'):
                require('1.1.1.1 8.8.8.8' in corefile, "Resource expectation failed: '1.1.1.1 8.8.8.8' in corefile")
                require(not container['resources'].get('limits'), "Resource expectation failed: not container['resources'].get('limits')")
            else:
                require(container['resources']['limits']['memory'] == '64Mi', "Resource expectation failed: container['resources']['limits']['memory'] == '64Mi'")
        elif component == 'localpath':
            require(container['image'] == 'docker.io/rancher/local-path-provisioner:v0.0.36', "Resource expectation failed: container['image'] == 'docker.io/rancher/local-path-provisioner:v0.0.36'")
            storage = objects['storageclasses'][0]
            require(storage['provisioner'] == 'rancher.io/local-path', "Resource expectation failed: storage['provisioner'] == 'rancher.io/local-path'")
            require(storage['reclaimPolicy'] == 'Retain', "Resource expectation failed: storage['reclaimPolicy'] == 'Retain'")
            require(storage['volumeBindingMode'] == 'WaitForFirstConsumer', "Resource expectation failed: storage['volumeBindingMode'] == 'WaitForFirstConsumer'")
            require(storage['metadata']['annotations']['storageclass.kubernetes.io/is-default-class'] == 'true', "Resource expectation failed: storage['metadata']['annotations']['storageclass.kubernetes.io/is-default-class'] == 'true'")
            data = objects['configmaps'][0]['data']
            config = json.loads(data['config.json'])
            expected = {'sharedFileSystemPath': '/fixture/shared'} if variant == 'shared' else {'nodePathMap': [{'node': 'DEFAULT_PATH_FOR_NON_LISTED_NODES', 'paths': ['/fixture/storage']}]}
            require(config == expected, 'Resource expectation failed: config == expected')
            require('image: busybox' in data['helperPod.yaml'], "Resource expectation failed: 'image: busybox' in data['helperPod.yaml']")
        elif component == 'portainer':
            require(container['image'] == 'portainer/agent:fixture', "Resource expectation failed: container['image'] == 'portainer/agent:fixture'")
            data = objects['configmaps'][0]['data']
            require(data['EDGE_ASYNC'] == str(variant == 'async').lower(), "Resource expectation failed: data['EDGE_ASYNC'] == str(variant == 'async').lower()")
            require(data['EDGE_ID'] == 'fixture-id', "Resource expectation failed: data['EDGE_ID'] == 'fixture-id'")
            require(data['EDGE_SECRET'] == 'fixture-secret', "Resource expectation failed: data['EDGE_SECRET'] == 'fixture-secret'")
            require(objects['secrets'][0]['stringData']['edge.key'] == 'fixture-key', "Resource expectation failed: objects['secrets'][0]['stringData']['edge.key'] == 'fixture-key'")
            require(objects['services'][0]['spec']['clusterIP'] == 'None', "Resource expectation failed: objects['services'][0]['spec']['clusterIP'] == 'None'")
        elif component == 'd2k':
            require(deployment['metadata']['namespace'] == 'fixture-d2k', "Resource expectation failed: deployment['metadata']['namespace'] == 'fixture-d2k'")
            require(container['image'] == 'fixture/d2k:fixed', "Resource expectation failed: container['image'] == 'fixture/d2k:fixed'")
            require(objects['services'][0]['spec']['ports'][0]['port'] == 2376, "Resource expectation failed: objects['services'][0]['spec']['ports'][0]['port'] == 2376")
            env = {e['name']: e for e in container['env']}
            require(env['D2K_NAMESPACE']['valueFrom']['fieldRef']['fieldPath'] == 'metadata.namespace', "Resource expectation failed: env['D2K_NAMESPACE']['valueFrom']['fieldRef']['fieldPath'] == 'metadata.namespace'")
            require(env['D2K_PORT']['value'] == '2376', "Resource expectation failed: env['D2K_PORT']['value'] == '2376'")
            require(env['D2K_SWARM_MODE']['value'] == 'true', "Resource expectation failed: env['D2K_SWARM_MODE']['value'] == 'true'")
            require(pod['volumes'][0]['secret']['secretName'] == 'd2k-tls', "Resource expectation failed: pod['volumes'][0]['secret']['secretName'] == 'd2k-tls'")
            require(objects['secrets'][0]['type'] == 'kubernetes.io/tls', "Resource expectation failed: objects['secrets'][0]['type'] == 'kubernetes.io/tls'")
            require(set(objects['secrets'][0]['data']) == {'tls.crt', 'tls.key'}, "Resource expectation failed: set(objects['secrets'][0]['data']) == {'tls.crt', 'tls.key'}")
            require(container['volumeMounts'][0]['readOnly'] is True, "Resource expectation failed: container['volumeMounts'][0]['readOnly'] is True")
        else:
            raise ValueError('unexpected component')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    verify([r for name in ('coredns', 'localpath', 'portainer', 'd2k') for r in json.loads((args.directory / (name + '.json')).read_text())])
    print('9 resource variants passed independent semantic checks')
