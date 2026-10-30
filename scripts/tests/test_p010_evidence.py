import copy
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('evidence', pathlib.Path(__file__).parents[1] / 'qualify-p010-evidence.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)


class EvidenceTests(unittest.TestCase):
    def complete(self, source='simulated'):
        doc = e.template(source)
        doc['versions'] = {key: 'test-1.0' for key in e.VERSIONS}
        for i, row in enumerate(doc['stages']):
            row.update(outcome='PASS', timestamp_utc='2026-10-30T00:00:00Z',
                       evidence_ref=f'proof-{i:03}.json', error_class='none',
                       actual_bytes=row['expected_bytes'])
        return doc

    def test_template_is_incomplete(self):
        self.assertTrue(e.validate(e.template()))
        with self.assertRaises(ValueError):
            e.validate(e.template(), True)

    def test_simulation_never_satisfies_physical_gate(self):
        self.assertTrue(e.validate(self.complete()))
        with self.assertRaises(ValueError):
            e.validate(self.complete(), True)
        self.assertTrue(e.validate(self.complete('physical'), True))

    def test_reject_incomplete_and_misclassified_evidence(self):
        doc = self.complete('physical')
        mutations = [lambda d: d['stages'].pop(),
                     lambda d: d['stages'].append(copy.deepcopy(d['stages'][0])),
                     lambda d: d['stages'][0].update(domain='crypto'),
                     lambda d: d['stages'][0].update(timestamp_utc=None),
                     lambda d: d['stages'][0].update(evidence_ref=None),
                     lambda d: d.update(authorization='approved'),
                     lambda d: d.update(raw_envelope='secret'),
                     lambda d: d['stages'][0].update(evidence_ref='../private.json'),
                     lambda d: d['stages'][0].update(outcome='UNKNOWN'),
                     lambda d: d['stages'][0].update(elapsed_ms=-1),
                     lambda d: d['stages'][0].update(actual_bytes=True),
                     lambda d: d['stages'][0].update(error_class='crypto'),
                     lambda d: d['versions'].update(watchos='unavailable'),
                     lambda d: d['stages'][5].update(actual_bytes=22),
                     lambda d: d['stages'][0].update(timestamp_utc='2026-10-30T00:00:00')]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                changed = copy.deepcopy(doc)
                mutate(changed)
                with self.assertRaises(ValueError):
                    e.validate(changed, True)

    def test_failure_is_valid_evidence_but_not_acceptance(self):
        doc = self.complete('physical')
        doc['stages'][0].update(outcome='FAIL', error_class='registration')
        self.assertTrue(e.validate(doc))
        with self.assertRaises(ValueError):
            e.validate(doc, True)
