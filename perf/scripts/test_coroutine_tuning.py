import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("tune_coroutines.py")
spec = importlib.util.spec_from_file_location("tune_coroutines", SCRIPT)
tuning = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tuning)


class CoroutineTuningTests(unittest.TestCase):
    def test_anchor_segments_restart_the_poll_budget(self):
        source = tuning.call_source(8, 3, 64, 65536)
        self.assertEqual(source.count("LLG_CO_CALL_ANCHOR("), 2)
        self.assertEqual(source.count("LLG_CO_CALL(co"), 6)
        self.assertIn("&desc_4, &F->child.an", source)
        self.assertIn("&desc_8, &F->child.an", source)

    def test_zero_poll_budget_anchors_every_edge(self):
        source = tuning.call_source(4, 0, 64, 65536)
        self.assertEqual(source.count("LLG_CO_CALL_ANCHOR("), 4)
        self.assertNotIn("LLG_CO_CALL(co", source)

    def test_embed_limit_uses_complete_leaf_frame_size(self):
        arena = tuning.call_source(1, 3, 4096, 4096)
        embedded = tuning.call_source(1, 3, 4096, 16384)
        self.assertIn("LLG_CO_CALL_ARENA", arena)
        self.assertNotIn("LLG_CO_CALL_ARENA", embedded)
        self.assertIn("frame_1 child", embedded)

    def test_caller_payload_separates_headers_without_changing_leaf_size(self):
        source = tuning.call_source(2, 3, 256, 65536, caller_bytes=64)
        self.assertEqual(source.count("unsigned char caller_payload[64]"), 2)
        self.assertEqual(source.count("unsigned char payload[256]"), 1)

    def test_frames_and_continuations_survive_repeated_yields(self):
        source = tuning.call_source(2, 1, 256, 65536)
        self.assertIn("uint64_t remaining", source)
        self.assertIn("unsigned char payload[256]", source)
        self.assertIn("LLG_CO_SUSPEND(co, ch, 1)", source)
        self.assertEqual(source.count("LLG_CO_RESUME_CASE(1)"), 3)


if __name__ == "__main__":
    unittest.main()
