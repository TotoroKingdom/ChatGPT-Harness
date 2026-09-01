# 核心技术架构V1

### 0-架构概览
                       Agent Layer
                           │
                       ChatGPT
                           │
                           │ MCP
                           ↓
                ┌────────────────────┐
                │ Transport Adapter  │
                │   tunnel-client    │
                └─────────┬──────────┘
                          │
                          ↓
                ┌────────────────────┐
                │  Harness Runtime   │
                │                    │
                │ Tool Registry      │
                │ Validation         │
                │ Policy Engine      │
                │ Approval Manager   │
                │ Executor           │
                │ Audit              │
                └─────────┬──────────┘
                          │
                   Local Machine
                          ↑
                          │ IPC
                ┌─────────┴──────────┐
                │      Tauri UI      │
                │                    │
                │ Config             │
                │ Approval           │
                │ Activity / Log     │
                └────────────────────┘

### 1-目录结构
ChatGPT-Harness/

├─ runtime/
├─ ui/
├─ tunnel-client/
├─ docs/
└─ README.md

runtime/

├─ protocol/
│   └─ MCP
│
├─ tools/
│   ├─ bash
│   ├─ read_file
│   ├─ write_file
│   ├─ edit_file
│   └─ glob
│
├─ policy/
│   ├─ deny
│   ├─ ask
│   └─ allow
│
├─ approval/
│
├─ executor/
│
├─ audit/
│
├─ config/
│
└─ runtime

### 2-原则
1. ChatGPT owns the agent loop.

2. Harness owns local execution.

3. All execution must pass through Policy Engine.

4. Permission is a security boundary, not a hook.

5. Bash is an escape hatch and is treated as high risk.

6. UI is a control plane; Runtime must not depend on UI.