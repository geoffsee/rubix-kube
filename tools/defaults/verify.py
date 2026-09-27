#!/usr/bin/env python3
"""Independent source-reviewed assertions plus exact frozen output comparison."""
import argparse
import hashlib
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent

def load_json(raw):
    def pairs(items):
        result={}
        for key,value in items:
            if key in result:raise ValueError('duplicate JSON key: '+key)
            result[key]=value
        return result
    def constant(value):raise ValueError('non-JSON constant: '+value)
    return json.loads(raw,object_pairs_hook=pairs,parse_constant=constant)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(value):
    require(value['source_revision']=='96cb9ab4201d88ce5e549fde047a686171838fdb','source revision')
    require(value['go_version']=='go1.26.8','toolchain')
    require(value['platform']=='linux/arm64','unqualified platform')
    require((value['emulation_version'],value['minimum_compatibility_version'])==('1.35','1.34'),'gate versions')
    require(value['feature_overrides']=={},'unexpected overrides')
    require(set(value['cases'])=={'zero','explicit'},'case inventory')
    zero=value['cases']['zero'];explicit=value['cases']['explicit']
    # Directly reviewed official pkg/*/apis/config/*/defaults.go assignments.
    k=zero['kubelet'];p=zero['proxy'];c=zero['controller']
    require(k['enableServer'] is True and k['port']==10250,'kubelet server default')
    require(k['authentication']['anonymous']['enabled'] is False,'anonymous authentication default')
    require(k['authorization']['mode']=='Webhook','authorization default')
    require(k['healthzPort']==10248 and k['healthzBindAddress']=='127.0.0.1','health defaults')
    require(p['bindAddress']=='0.0.0.0','proxy bind default')
    require(p['clientConnection']['qps']==5 and p['clientConnection']['burst']==10,'proxy client limits')
    require(p['iptables']['syncPeriod']=='30s','iptables sync default')
    require(c['Generic']['ClientConnection']['qps']==20 and c['Generic']['ClientConnection']['burst']==30,'controller client limits')
    require(explicit['kubelet']['enableServer'] is False,'explicit false lost')
    require(explicit['kubelet']['port']==10260 and explicit['kubelet']['readOnlyPort']==1234,'explicit ports lost')
    require(explicit['proxy']['bindAddress']=='192.0.2.9' and explicit['proxy']['clientConnection']['qps']==17,'explicit proxy values lost')
    require(explicit['controller']['Generic']['ClientConnection']['qps']==19,'explicit controller QPS lost')
    cloud=c['KubeCloudShared']
    require(cloud['NodeMonitorPeriod']=='5s' and cloud['RouteReconciliationPeriod']=='10s','nested cloud duration defaults')
    require(cloud['ClusterName']=='kubernetes' and cloud['ConfigureCloudRoutes'] is True,'nested cloud identity/routes defaults')
    configured=explicit['controller']['KubeCloudShared']
    require(configured['ClusterName']=='fixture-cluster' and configured['ConfigureCloudRoutes'] is False,'nested explicit cloud values lost')
    require(configured['NodeMonitorPeriod']=='7s' and configured['RouteReconciliationPeriod']=='10s','nested explicit/default cloud durations')
    require(explicit['kubelet']['reservedMemory']==[{'numaNode':0,'limits':{'memory':'1001m'}}],'generated ResourceList milli rounding skipped')
    gates=value['registered_feature_gates']
    # Independently reviewed pkg/features/kube_features.go versioned entries.
    require(gates['RotateKubeletServerCertificate']['enabled'] is True,'rotation gate default')
    require(gates['RotateKubeletServerCertificate']['specs']==[
        {'default':False,'locked':False,'stage':'ALPHA','version':'1.7','minimum_compatibility':''},
        {'default':True,'locked':False,'stage':'BETA','version':'1.12','minimum_compatibility':''}], 'rotation gate history')
    sidecar=gates['SidecarContainers']
    require(sidecar['enabled'] is True,'sidecar enabled default')
    require(sidecar['specs'][-1]=={'default':True,'locked':True,'stage':'','version':'1.33','minimum_compatibility':''},'sidecar GA history')


def verify_apiserver(value):
    verify(value)
    options=value['apiserver_options']
    require(options['construction_only'] is True,'unexpected lifecycle scope')
    require(options['system_namespaces']==['kube-system','kube-public','default','kube-node-lease'],'system namespace defaults')
    require(options['kubelet_preferred_address_types']==['Hostname','InternalDNS','InternalIP','ExternalDNS','ExternalIP'],'kubelet address preference')
    require(options['service_node_port_range']=='30000-32767','service port range')
    expected={'secure-port':'6443','allow-privileged':'false','kubelet-port':'10250',
              'kubelet-timeout':'5s','service-node-port-range':'30000-32767',
              'event-ttl':'1h0m0s','storage-media-type':'application/vnd.kubernetes.protobuf',
              'watch-cache':'true','apiserver-count':'1','advertise-address':'<nil>',
              'external-hostname':'','anonymous-auth':'true','authorization-mode':'[]'}
    for name,default in expected.items():
        require(options['flags'][name]['default']==default,'API-server default '+name)


def differences(before,after,path=''):
    if type(before) is not type(after):
        yield {'path':path,'before':before,'after':after}
    elif isinstance(before,dict):
        for key in sorted(before.keys()|after.keys()):
            if key not in before:yield {'path':path+'/'+key,'added':after[key]}
            elif key not in after:yield {'path':path+'/'+key,'removed':before[key]}
            else:yield from differences(before[key],after[key],path+'/'+key)
    elif isinstance(before,list) and len(before)==len(after):
        for index,(old,new) in enumerate(zip(before,after)):
            yield from differences(old,new,path+'/'+str(index))
    elif before!=after:yield {'path':path,'before':before,'after':after}


def load_expected(path, validator=verify, digest_key='expected_sha256'):
    raw=path.read_bytes()
    expected=load_json(raw.decode())
    validator(expected)
    provenance=load_json((HERE/'provenance.json').read_text())
    require(hashlib.sha256(raw).hexdigest()==provenance[digest_key], 'expected fixture provenance mismatch')
    return expected


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path);parser.add_argument('--expected',type=Path,default=HERE/'expected.json');args=parser.parse_args()
    actual=load_json(args.capture.read_text())
    validator=verify_apiserver if 'apiserver_options' in actual else verify
    digest_key='apiserver_expected_sha256' if 'apiserver_options' in actual else 'expected_sha256'
    validator(actual)
    delta=list(differences(load_expected(args.expected,validator,digest_key),actual))
    print(json.dumps({'status':'drift' if delta else 'unchanged','changes':delta},indent=2))
    return bool(delta)

if __name__=='__main__':raise SystemExit(main())
