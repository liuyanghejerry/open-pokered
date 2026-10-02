"""Run seeded subsystem scenarios with the same deterministic driver as the chain."""
from pathlib import Path
import runpy

runpy.run_path(str(Path(__file__).with_name("run_first_clear.py")), run_name="audit_driver")
import scenarios

if __name__ == "__main__":
    scenarios.main()
