
nodes = []

# 1. Init node
nodes.append("""  - id: "init"
    type: "script"
    params:
      language: "python"
      script: "print('Starting distributed test')"
""")

# 2. 50 parallel nodes
needs_list = []
for i in range(50):
    node_id = f"task_{i}"
    needs_list.append(f'"{node_id}"')
    nodes.append(f"""  - id: "{node_id}"
    type: "script"
    needs: ["init"]
    params:
      language: "python"
      script: "import time; time.sleep(2); print('Task {i} completed')"
""")

# 3. Summary node
needs_str = ", ".join(needs_list)
nodes.append(f"""  - id: "summary"
    type: "script"
    needs: [{needs_str}]
    params:
      language: "python"
      script: "print('All tasks completed')"
""")

yaml_content = f"""name: "True Distributed Test"
version: "1.0"
global: {{}}
nodes:
{"".join(nodes)}
"""

with open("benchmarks/distributed_flat.yaml", "w") as f:
    f.write(yaml_content)

print("Generated benchmarks/distributed_flat.yaml")
