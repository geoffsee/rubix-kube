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


def verify(fixture):
    cases=fixture['cases']
    for kind in ('pod','service','custom'):
        assert typed_equal(cases[kind+'-create'],cases[kind+'-read'])
    pod=cases['pod-read'];metadata=pod['metadata']
    assert metadata['name']=='quantities' and metadata['namespace']=='serialization-fixture'
    assert metadata['labels']=={'fixture':'json'}
    assert metadata['annotations']=={'example.test/text':'literal: null'}
    assert 'finalizers' not in metadata and 'nodeSelector' not in pod['spec']
    assert 'ownerReferences' not in metadata
    assert pod['spec']['automountServiceAccountToken'] is False
    assert 'volumes' not in pod['spec']
    resources=pod['spec']['containers'][0]['resources']
    assert typed_equal(resources,{'requests':{'cpu':'500m','memory':'1536Mi','ephemeral-storage':'1e3'},
                                   'limits':{'cpu':'1','memory':'2Gi'}})
    ports=cases['service-read']['spec']['ports']
    assert type(ports[0]['targetPort']) is str and ports[0]['targetPort']=='http'
    assert type(ports[1]['targetPort']) is int and ports[1]['targetPort']==8080
    assert 'annotations' not in cases['service-read']['metadata']
    assert typed_equal(cases['custom-read']['spec'],{'unknown':{'metadata':{'uid':'user-value'},
        'null':None,'bool':False,'integer':17,'decimal':1.25,'list':[None,True,'7',7,{'nested':'value'}]}})
    assert [e['type'] for e in fixture['watch']]==['ADDED','MODIFIED','DELETED']
    for event,value in zip(fixture['watch'],('first','second','second')):
        assert event['object']['kind']=='ConfigMap' and event['object']['apiVersion']=='v1'
        assert event['object']['metadata']['name']=='watched'
        assert event['object']['data']=={'value':value}
