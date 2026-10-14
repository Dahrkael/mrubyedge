import tempfile
from pathlib import Path
import unittest

from mruby_spec_report import summarize, read_events, markdown, html_report


class Reports(unittest.TestCase):
    def test_nested_asserts_and_failures_survive_a_crash(self):
        result = summarize([
            ['phase', 'execute', 'test.rb'], ['start', 1, 'outer'],
            ['start', 2, 'inner'], ['check_failure', 0, 'bad', 'expected 1'],
            ['fail', 2, None], ['start', 3, 'crashes'],
        ], -6, False, '')
        self.assertEqual(result['status'], 'aborted')
        self.assertEqual(result['counts']['interrupted'], 2)
        self.assertEqual(result['counts']['fail'], 1)
        self.assertEqual(result['assertions'][1]['parent_id'], 1)
        self.assertEqual(result['assertions'][1]['details'], ['bad expected 1'])
        self.assertFalse(result['completed'])

    def test_terminal_states(self):
        for phase, code, timeout, log, expected in [
            ('completed', 0, False, '', 'passed'),
            ('compile', 101, False, '', 'compile_error'),
            ('execute', 101, False, "thread panicked at x", 'panic'),
            ('runtime_error', 101, False, '', 'runtime_error'),
            ('returned', 101, False, '', 'premature_return'),
            ('execute', None, True, '', 'timeout'),
            ('execute', 0, False, '', 'incomplete'),
        ]:
            with self.subTest(expected=expected):
                self.assertEqual(summarize([['phase', phase, 'test.rb']], code, timeout, log)['status'], expected)
        result = summarize([['start', 1, 'bad'], ['error', 1, 'oops'], ['phase', 'completed', 'test.rb']], 101, False, '')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['counts']['error'], 1)

    def test_unfinished_assert_is_not_a_passing_file(self):
        result = summarize([['start', 1, 'unfinished'], ['phase', 'completed', 'test.rb']], 0, False, '')
        self.assertEqual(result['status'], 'incomplete')
        self.assertEqual(result['counts']['interrupted'], 1)

    def test_truncated_last_event_keeps_completed_events(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'events.jsonl'
            path.write_text('["start",1,"name"]\n["pass",1,null]\n["start",')
            self.assertEqual(len(read_events(path)), 2)

    def test_renderers_escape_descriptions_and_details(self):
        item = summarize([['start', 1, '<script>x</script>|[link]'], ['error', 1, 'a\nb'],
                          ['phase', 'completed', 'example.rb']], 101, False, '')
        item.update(file='example.rb', elapsed_seconds=1.5, source='source.rb', log='raw.log', events='events.jsonl')
        report = {'metadata': {'commit': 'abcd', 'dirty': False, 'generated_at': 'now', 'features': '',
                               'default_features': True, 'upstream_revision': '1234'},
                  'files': [item], 'file_counts': {'failed': 1}}
        rendered = html_report(report)
        self.assertNotIn('<script>', rendered)
        self.assertIn('&lt;script&gt;', rendered)
        rendered = markdown(report)
        self.assertNotIn('<script>', rendered)
        self.assertIn(r'\|\[link\]', rendered)
        self.assertIn('a<br>b', rendered)


if __name__ == '__main__':
    unittest.main()
