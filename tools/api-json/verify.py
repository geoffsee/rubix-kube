"""Independent assertions; only server-owned volatile top-level metadata is normalized."""
import copy

VOLATILE = ('uid', 'resourceVersion', 'creationTimestamp', 'managedFields')


def normalize(resource):
    result = copy.deepcopy(resource)
    for name in VOLATILE:
        result.get('metadata', {}).pop(name, None)
    return result


def typed_equal(a, b):
    if type(a) is not type(b): return False
    if isinstance(a, dict): return a.keys() == b.keys() and all(typed_equal(a[k], b[k]) for k in a)
    if isinstance(a, list): return len(a) == len(b) and all(typed_equal(x,y) for x,y in zip(a,b))
    return a == b


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(fixture):
    cases=fixture['cases']
    for kind in ('pod','service','custom'):
        require(typed_equal(cases[kind + '-create'], cases[kind + '-read']), "API JSON expectation failed: typed_equal(cases[kind + '-create'], cases[kind + '-read'])")
    pod=cases['pod-read'];metadata=pod['metadata']
    require(metadata['name'] == 'quantities' and metadata['namespace'] == 'serialization-fixture', "API JSON expectation failed: metadata['name'] == 'quantities' and metadata['namespace'] == 'serialization-fixture'")
    require(metadata['labels'] == {'fixture': 'json'}, "API JSON expectation failed: metadata['labels'] == {'fixture': 'json'}")
    require(metadata['annotations'] == {'example.test/text': 'literal: null'}, "API JSON expectation failed: metadata['annotations'] == {'example.test/text': 'literal: null'}")
    require('finalizers' not in metadata and 'nodeSelector' not in pod['spec'], "API JSON expectation failed: 'finalizers' not in metadata and 'nodeSelector' not in pod['spec']")
    require('ownerReferences' not in metadata, "API JSON expectation failed: 'ownerReferences' not in metadata")
    require(pod['spec']['automountServiceAccountToken'] is False, "API JSON expectation failed: pod['spec']['automountServiceAccountToken'] is False")
    require('volumes' not in pod['spec'], "API JSON expectation failed: 'volumes' not in pod['spec']")
    resources=pod['spec']['containers'][0]['resources']
    require(typed_equal(resources, {'requests': {'cpu': '500m', 'memory': '1536Mi', 'ephemeral-storage': '1e3'}, 'limits': {'cpu': '1', 'memory': '2Gi'}}), "API JSON expectation failed: typed_equal(resources, {'requests': {'cpu': '500m', 'memory': '1536Mi', 'ephemeral-storage': '1e3'}, 'limits': {'cpu': '1', 'memory': '2Gi'}})")
    ports=cases['service-read']['spec']['ports']
    require(type(ports[0]['targetPort']) is str and ports[0]['targetPort'] == 'http', "API JSON expectation failed: type(ports[0]['targetPort']) is str and ports[0]['targetPort'] == 'http'")
    require(type(ports[1]['targetPort']) is int and ports[1]['targetPort'] == 8080, "API JSON expectation failed: type(ports[1]['targetPort']) is int and ports[1]['targetPort'] == 8080")
    require('annotations' not in cases['service-read']['metadata'], "API JSON expectation failed: 'annotations' not in cases['service-read']['metadata']")
    require(typed_equal(cases['custom-read']['spec'], {'unknown': {'metadata': {'uid': 'user-value'}, 'null': None, 'bool': False, 'integer': 17, 'decimal': 1.25, 'list': [None, True, '7', 7, {'nested': 'value'}]}}), "API JSON expectation failed: typed_equal(cases['custom-read']['spec'], {'unknown': {'metadata': {'uid': 'user-value'}, 'null': None, 'bool': False, 'integer': 17, 'decimal': 1.25, 'list': [None, True, '7', 7, {'nested': 'value'}]}})")
    require([e['type'] for e in fixture['watch']] == ['ADDED', 'MODIFIED', 'DELETED'], "API JSON expectation failed: [e['type'] for e in fixture['watch']] == ['ADDED', 'MODIFIED', 'DELETED']")
    for event,value in zip(fixture['watch'],('first','second','second')):
        require(event['object']['kind'] == 'ConfigMap' and event['object']['apiVersion'] == 'v1', "API JSON expectation failed: event['object']['kind'] == 'ConfigMap' and event['object']['apiVersion'] == 'v1'")
        require(event['object']['metadata']['name'] == 'watched', "API JSON expectation failed: event['object']['metadata']['name'] == 'watched'")
        require(event['object']['data'] == {'value': value}, "API JSON expectation failed: event['object']['data'] == {'value': value}")
