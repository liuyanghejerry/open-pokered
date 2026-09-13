"""openpokered — Python adapter for the pokered AI-agent research environment (M6).

Layers:
    client.py    typed debug-API client (AgentClient)
    env.py       OpenPokeredEnv gym-style wrapper (reward stays experiment-side)
    tasks.py     task-spec loading/validation (+ tasks/*.json)
    metrics.py   per-run metrics JSONL
    skills.py    reusable skill actions (battle loop, heal, wild wins)
    oracle.py    rule-based oracle planner
    run_task.py  runner CLI
"""
