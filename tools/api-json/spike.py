"""Actual official API-server JSON observations, independent of Rust bindings."""
import copy
import hashlib
import json
import platform
import ssl
import subprocess
import time
import traceback
import urllib.error
import urllib.request

import component as c
from verify import normalize, verify

records = []


def request(name, method, path, body=None, expected=200, identity="admin"):
    data = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    req = urllib.request.Request("https://127.0.0.1:6443" + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        response = urllib.request.urlopen(req, context=c.context(identity), timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(2 * 1024 * 1024 + 1)
        c.check(len(raw) <= 2 * 1024 * 1024, "bounded response " + name)
        status = response.code
    parsed = json.loads(raw) if raw.startswith(b"{") else raw.decode()
    records.append({"name": name, "method": method, "path": path, "request": body,
                    "status": status, "raw_response": raw.decode(), "response": parsed})
    c.check(status == expected, f"HTTP {name}: {status}, expected {expected}")
    return parsed


def datastore_negative(identity):
    command = ["openssl", "s_client", "-connect", "127.0.0.1:2379", "-tls1_2",
               "-verify_return_error", "-verify_ip", "127.0.0.1", "-CAfile", "/state/datastore-ca.crt", "-brief"]
    if identity:
        command += ["-cert", "/state/" + identity + ".crt", "-key", "/state/" + identity + ".key"]
    result = subprocess.run(command, input=b"", stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
    c.report.setdefault("datastore_tls", []).append({"identity": identity or "absent",
        "exit_code": result.returncode, "diagnostic": result.stderr.decode()})
    c.check(result.returncode != 0, "datastore rejects " + str(identity))


def run():
    inputs = json.loads(c.Path('/experiment/inputs.json').read_text())
    arch = {"aarch64": "arm64", "x86_64": "amd64"}[platform.machine()]
    c.report.update(inputs=inputs, architecture=arch, kernel=platform.release(),
                    fixture="isolated unprivileged Docker; API and datastore only",
                    source_sha256={name: hashlib.sha256(c.Path('/experiment', name).read_bytes()).hexdigest()
                                   for name in ('spike.py', 'component.py', 'verify.py', 'inputs.json')})
    for name, artifact in inputs['artifacts'][arch].items():
        c.check(hashlib.sha256(c.Path('/usr/local/bin', name).read_bytes()).hexdigest() == artifact['sha256'],
                'runtime digest ' + name)
    c.credentials()
    c.ready(1)
    datastore_negative(None)
    datastore_negative('admin')  # Kubernetes administrative CA is not the datastore CA.
    request('anonymous-denied', 'GET', '/api/v1/namespaces', expected=401, identity=None)
    request('rbac-denied', 'GET', '/api/v1/namespaces', expected=403, identity='unprivileged')
    request('namespace', 'POST', '/api/v1/namespaces', {'apiVersion':'v1','kind':'Namespace',
            'metadata':{'name':'serialization-fixture'}}, 201)
    prefix='/api/v1/namespaces/serialization-fixture'
    request('service-account','POST',prefix+'/serviceaccounts',
            {'apiVersion':'v1','kind':'ServiceAccount','metadata':{'name':'default'}},201)
    pod={'apiVersion':'v1','kind':'Pod','metadata':{'name':'quantities','labels':{'fixture':'json'},
        'annotations':{'example.test/text':'literal: null'}, 'finalizers':None},
        'spec':{'automountServiceAccountToken':False,'containers':[{'name':'main','image':'example.invalid/fixture:v1','resources':{
            'requests':{'cpu':'0.5','memory':'1.5Gi','ephemeral-storage':'1e3'},
            'limits':{'cpu':'1','memory':'2Gi'}}}], 'nodeSelector':None}}
    request('pod-create','POST',prefix+'/pods',pod,201)
    request('pod-read','GET',prefix+'/pods/quantities')
    service={'apiVersion':'v1','kind':'Service','metadata':{'name':'ports','annotations':None},
        'spec':{'clusterIP':'None','selector':{'fixture':'json'},'ports':[
        {'name':'named','port':80,'targetPort':'http'},{'name':'numeric','port':81,'targetPort':8080}]}}
    request('service-create','POST',prefix+'/services',service,201)
    request('service-read','GET',prefix+'/services/ports')
    crd={'apiVersion':'apiextensions.k8s.io/v1','kind':'CustomResourceDefinition',
        'metadata':{'name':'samples.fixture.rubix.test'},'spec':{'group':'fixture.rubix.test','scope':'Namespaced',
        'names':{'plural':'samples','singular':'sample','kind':'Sample'},'versions':[{'name':'v1',
        'served':True,'storage':True,'schema':{'openAPIV3Schema':{'type':'object','properties':{
            'spec':{'type':'object','x-kubernetes-preserve-unknown-fields':True}}}}}]}}
    request('crd-create','POST','/apis/apiextensions.k8s.io/v1/customresourcedefinitions',crd,201)
    deadline=time.monotonic()+30
    while time.monotonic()<deadline:
        value=request('crd-ready','GET','/apis/apiextensions.k8s.io/v1/customresourcedefinitions/samples.fixture.rubix.test')
        if any(x['type']=='Established' and x['status']=='True' for x in value.get('status',{}).get('conditions',[])):break
        time.sleep(.2)
    else:raise RuntimeError('CRD readiness timeout')
    custom={'apiVersion':'fixture.rubix.test/v1','kind':'Sample','metadata':{'name':'arbitrary'},
            'spec':{'unknown':{'metadata':{'uid':'user-value'},'null':None,'bool':False,'integer':17,
                               'decimal':1.25,'list':[None,True,'7',7,{'nested':'value'}]}}}
    custom_path='/apis/fixture.rubix.test/v1/namespaces/serialization-fixture/samples'
    request('custom-create','POST',custom_path,custom,201)
    request('custom-read','GET',custom_path+'/arbitrary')
    before=request('watch-before','GET',prefix+'/configmaps')['metadata']['resourceVersion']
    item={'apiVersion':'v1','kind':'ConfigMap','metadata':{'name':'watched'},'data':{'value':'first'}}
    created=request('watch-create','POST',prefix+'/configmaps',item,201)
    updated=copy.deepcopy(created);updated['data']['value']='second'
    request('watch-update','PUT',prefix+'/configmaps/watched',updated)
    request('watch-delete','DELETE',prefix+'/configmaps/watched')
    url='https://127.0.0.1:6443'+prefix+'/configmaps?watch=true&timeoutSeconds=10&fieldSelector=metadata.name%3Dwatched&resourceVersion='+before
    lines=[]
    with urllib.request.urlopen(url,context=c.context('admin'),timeout=15) as response:
        for _ in range(3):
            line=response.readline(1024*1024+1)
            if not line or len(line)>1024*1024:raise RuntimeError('missing/oversized watch event')
            lines.append(line.decode())
    events=[json.loads(line) for line in lines]
    fixture={'schema_version':1,'cases':{name:normalize(next(r['response'] for r in records if r['name']==name))
             for name in ('pod-create','pod-read','service-create','service-read','custom-create','custom-read')},
             'watch':[{'type':event['type'],'object':normalize(event['object'])} for event in events]}
    c.report['raw_watch_lines']=lines
    c.report['fixture']=fixture
    verify(fixture)
    c.report['status']='passed'


try:
    run()
except BaseException as error:
    c.report['error']=str(error)
    c.report['traceback']=traceback.format_exc()
finally:
    c.report['http']=records
    try:c.shutdown()
    except BaseException as error:
        c.report['status']='failed';c.report['cleanup_error']=str(error)
    (c.EVIDENCE/'result.json').write_text(json.dumps(c.report,indent=2)+'\n')
    if c.report.get('fixture'):
        (c.EVIDENCE/'fixtures.json').write_text(json.dumps(c.report['fixture'],indent=2)+'\n')
    print(c.report['status'],flush=True)
raise SystemExit(0 if c.report['status']=='passed' else 1)
