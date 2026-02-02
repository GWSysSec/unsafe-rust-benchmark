#!/usr/bin/env python3
import os
import subprocess
import glob
import sys

# Configuration
LLVM_BUILD_DIR = os.path.abspath("build/x86_64-unknown-linux-gnu/llvm/build")
BIN_DIR = os.path.join(LLVM_BUILD_DIR, "bin")
SVF_BIN_DIR = os.path.join(LLVM_BUILD_DIR, "tools/SVF/bin")
TEST_DIR = os.path.abspath("src/llvm-project/llvm/test/Analysis/SVF_Suite")

def setup_env():
    env = os.environ.copy()
    current_path = env.get("PATH", "")
    env["PATH"] = f"{SVF_BIN_DIR}:{BIN_DIR}:{current_path}"
    return env

def parse_run_line(line, filepath):
    # Format: ; RUN: command %s | FileCheck %s
    if "; RUN:" not in line:
        return None
    
    cmd_str = line.split("; RUN:")[1].strip()
    
    # Simple substitution
    cmd_str = cmd_str.replace("%s", filepath)
    
    return cmd_str

def run_test(filepath, env):
    with open(filepath, 'r') as f:
        lines = f.readlines()
        
    run_lines = [l for l in lines if "; RUN:" in l]
    if not run_lines:
        return True # No tests defined? skip
        
    for line in run_lines:
        cmd = parse_run_line(line, filepath)
        if not cmd:
            continue
            
        print(f"  Running: {cmd}")
        try:
            # Shell=True to handle pipes
            subprocess.run(cmd, env=env, shell=True, check=True, 
                         stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        except subprocess.CalledProcessError as e:
            print(f"  FAILED: {e}")
            if e.stderr:
                print(f"  STDERR: {e.stderr.decode('utf-8')}")
            return False
            
    return True

def main():
    if not os.path.exists(TEST_DIR):
        print(f"Test directory {TEST_DIR} does not exist. Did you run svf_to_lit.py?")
        sys.exit(1)
        
    tests = glob.glob(os.path.join(TEST_DIR, "**/*.ll"), recursive=True)
    tests.sort()
    
    print(f"Found {len(tests)} tests in {TEST_DIR}")
    
    passed = 0
    failed = 0
    env = setup_env()
    
    for t in tests:
        rel_path = os.path.relpath(t, TEST_DIR)
        print(f"TEST: {rel_path}")
        if run_test(t, env):
            print("  PASS")
            passed += 1
        else:
            print("  FAIL")
            failed += 1
            
    print("-" * 40)
    print(f"TOTAL: {len(tests)}")
    print(f"PASS:  {passed}")
    print(f"FAIL:  {failed}")
    
    if failed > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
