import sys
import json

def log(msg):
    sys.stderr.write(f"[MockServer] {msg}\n")
    sys.stderr.flush()

def main():
    log("Starting...")
    for line in sys.stdin:
        if not line.strip():
            continue
        
        try:
            req = json.loads(line)
            log(f"Received: {req}")
            
            method = req.get("method")
            msg_id = req.get("id")
            
            if method == "initialize":
                resp = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "mock-server",
                            "version": "1.0"
                        }
                    }
                }
                print(json.dumps(resp))
                sys.stdout.flush()
                
            elif method == "notifications/initialized":
                log("Initialized!")
                
            elif method == "tools/call":
                params = req.get("params", {})
                tool_name = params.get("name")
                args = params.get("arguments", {})
                
                result = {}
                if tool_name == "add":
                    a = args.get("a", 0)
                    b = args.get("b", 0)
                    result = {"content": [{"type": "text", "text": str(a + b)}]}
                elif tool_name == "echo":
                    result = {"content": [{"type": "text", "text": args.get("message", "")}]}
                else:
                    result = {"content": [{"type": "text", "text": "Unknown tool"}]}
                    
                resp = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": result
                }
                print(json.dumps(resp))
                sys.stdout.flush()
                
        except Exception as e:
            log(f"Error: {e}")

if __name__ == "__main__":
    main()
