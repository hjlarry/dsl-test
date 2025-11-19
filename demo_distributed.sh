#!/bin/bash

# 分布式执行演示脚本

echo "🚀 分布式工作流引擎演示"
echo "================================"
echo ""

# 清理旧进程
echo "🧹 清理旧进程..."
pkill -f "workflow-engine coordinator" || true
pkill -f "workflow-engine worker" || true
sleep 1

# 终端1:启动 Coordinator
echo "📋 步骤1: 启动 Coordinator (端口8080)"
echo "命令: cargo run --release -- coordinator -p 8080"
echo ""
echo "请在新终端运行上述命令，然后按回车继续..."
read dummy

# 终端2-4: 启动 3 个 Workers
echo ""
echo "👷 步骤2: 启动 3 个 Workers"
echo ""
echo "终端2: cargo run --release -- worker -i worker-1 -p 3001 -c http://localhost:8080"
echo "终端3: cargo run --release -- worker -i worker-2 -p 3002 -c http://localhost:8080"
echo "终端4: cargo run --release -- worker -i worker-3 -p 3003 -c http://localhost:8080"
echo ""
echo "请在3个新终端分别运行上述命令，然后按回车继续..."
read dummy

# 等待Workers注册
echo ""
echo "⏳ 等待Workers注册..."
sleep 3

# 检查Workers列表
echo ""
echo "✅ 查看已注册的Workers:"
curl -s http://localhost:8080/workers | jq .
echo ""

# 提交工作流
echo ""
echo "📤 步骤3: 生成并提交测试工作流 (50个并行节点)"
echo ""

# 生成测试文件
python3 benchmarks/gen_workflow.py

time ./target/release/workflow-engine submit -f benchmarks/distributed_flat.yaml -c http://localhost:8080

echo ""
echo "================================"
echo "✨ 演示完成！"
echo ""
echo "💡 提示:"
echo "  - 50个任务，每个耗时2秒"
echo "  - 单线程执行需要 100秒"
echo "  - 3机分布式执行仅需 ~4秒！"
echo "  - 真正的并行计算能力 🚀"
