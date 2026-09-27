"""Mutation controls for actual API-server serialization observations."""
import copy
import hashlib
import json
from pathlib import Path
import unittest

from verify import normalize, typed_equal, verify

HERE = Path(__file__).resolve().parent


class SerializationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = json.loads((HERE / 'fixtures.json').read_text())

    def test_official_server_observations_match_independent_expectations(self):
        verify(self.fixture)

    def mutate_both(self, name, mutation):
        for suffix in ('create','read'):
            mutation(self.fixture['cases'][name+'-'+suffix])
        with self.assertRaises(AssertionError): verify(self.fixture)

    def test_noncanonical_quantity_is_rejected_even_when_both_results_agree(self):
        self.mutate_both('pod',lambda p:p['spec']['containers'][0]['resources']['requests'].update(cpu='0.5'))

    def test_numeric_target_port_cannot_be_string(self):
        self.mutate_both('service',lambda s:s['spec']['ports'][1].update(targetPort='8080'))

    def test_omitted_metadata_cannot_reappear_as_null(self):
        self.mutate_both('pod',lambda p:p['metadata'].update(finalizers=None))

    def test_arbitrary_crd_boolean_is_not_integer_zero(self):
        self.mutate_both('custom',lambda c:c['spec']['unknown'].update(bool=0))

    def test_arbitrary_crd_null_cannot_be_dropped(self):
        self.mutate_both('custom',lambda c:c['spec']['unknown'].pop('null'))

    def test_watch_event_type_and_latest_deleted_value_matter(self):
        for index,change in [(0,{'type':'MODIFIED'}),(2,{'object':copy.deepcopy(self.fixture['watch'][0]['object'])})]:
            changed=copy.deepcopy(self.fixture);changed['watch'][index].update(change)
            with self.subTest(index=index),self.assertRaises(AssertionError):verify(changed)

    def test_normalization_removes_only_explicit_server_metadata(self):
        source={'metadata':{'uid':'volatile','creationTimestamp':'volatile','resourceVersion':'42',
                            'managedFields':[],'name':'kept','generation':3},
                'spec':{'metadata':{'uid':'user-value'},'creationTimestamp':'user-value'}}
        normalized=normalize(source)
        self.assertEqual(normalized,{'metadata':{'name':'kept','generation':3},'spec':source['spec']})
        self.assertEqual(source['metadata']['uid'],'volatile')

    def test_raw_observations_regenerate_the_frozen_fixture(self):
        result=json.loads((HERE/'evidence/result.json').read_text())
        cases={r['name']:r for r in result['http']}
        for name,expected in self.fixture['cases'].items():
            raw=json.loads(cases[name]['raw_response'])
            self.assertTrue(typed_equal(normalize(raw),expected))
        raw_events=[json.loads(line) for line in result['raw_watch_lines']]
        self.assertTrue(typed_equal([{'type':e['type'],'object':normalize(e['object'])} for e in raw_events],self.fixture['watch']))
        self.assertEqual(result['status'],'passed')
        self.assertTrue(all(not r['forced'] and not r['owned_group_remained'] and not r['error'] for r in result['shutdowns']))
        self.assertEqual({r['identity'] for r in result['datastore_tls']},{'absent','admin'})
        self.assertTrue(all(r['exit_code']!=0 for r in result['datastore_tls']))

    def test_durable_evidence_matches_provenance(self):
        provenance=json.loads((HERE/'provenance.json').read_text())
        for name,digest in provenance['durable_sha256'].items():
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)


if __name__=='__main__':unittest.main()
