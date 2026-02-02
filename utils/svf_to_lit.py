#!/usr/bin/env python3
import os
import subprocess
import argparse
import sys
import shutil

# Configuration
SVF_ROOT = "/home/oscar/Projects/pointeranalysis/SVF"
TEST_SUITE_SRC = os.path.join(SVF_ROOT, "Test-Suite", "src")
OUTPUT_DIR = os.path.abspath("src/llvm-project/llvm/test/Analysis/SVF_Suite")
HEADER_PATH = os.path.join(SVF_ROOT, "Test-Suite")

# SVF WPA Path (Assuming locally built for verification, but script generates lit tests using 'opt')
# In lit, we will use %opt or a custom tool substitution.
# For now, we generate standard RUN lines.

def ensure_dir(d):
    if not os.path.exists(d):
        os.makedirs(d)

def run_cmd(cmd):
    # print(f"CMD: {cmd}")
    subprocess.run(cmd, shell=True, check=True)

def generate_lit_checks(c_file):
    """
    Parses the C file to find MUSTALIAS/NOALIAS expectations and generates FileCheck lines.
    """
    checks = []
    with open(c_file, 'r') as f:
        lines = f.readlines()
    
    # Simple heuristic: scan for MACROS and map to generic checks
    # BUT, wpa output is not easily mapped line-by-line to source without debug info matching.
    # WPA output: "MayAlias: 10pts(p) 20pts(q)" or similar.
    # The current wpa output is very verbose statistics or bare "Alias analysis results".
    # 
    # CRITICAL: SVF's test suite relies on 'aliascheck.h' which prints nothing but can be used for runtime validation?
    # NO, SVF normally uses -valid-tests option or just standard analysis.
    # If we use `wpa -ander -print-aliases`, it prints alias pairs.
    
    # For this pass, we will generate a "Run and Don't Crash" test structure first.
    # Precision verification requires parsing the output of `wpa -print-aliases` and matching it to macro calls.
    pass
    return checks

def process_file(category, filename):
    base_name = os.path.splitext(filename)[0]
    c_path = os.path.join(TEST_SUITE_SRC, category, filename)
    
    out_cat_dir = os.path.join(OUTPUT_DIR, category)
    ensure_dir(out_cat_dir)
    
    ll_out = os.path.join(out_cat_dir, base_name + ".ll")
    
    # Compile to LLVM IR
    # We use system clang because in-tree is building
    try:
        cmd_compile = f"clang -S -emit-llvm -O0 -Xclang -disable-O0-optnone -g -I{HEADER_PATH} {c_path} -o {ll_out}"
        run_cmd(cmd_compile)
        
        # Optimize (mem2reg is usually required for SVF to work reasonably well)
        cmd_opt = f"./build/x86_64-unknown-linux-gnu/llvm/build/bin/opt -passes=mem2reg -S {ll_out} -o {ll_out}"
        run_cmd(cmd_opt)
        
        # Prepend lit RUN directive
        with open(ll_out, 'r') as f:
            content = f.read()
            
        header = f"""; RUN: wpa -ander -stat %s 2>&1 | FileCheck %s
; RUN: wpa -ander -print-pts %s 2>&1 | FileCheck %s --check-prefix=PTS
; CHECK: Andersen Pointer Analysis
; PTS: Points-To
"""
        with open(ll_out, 'w') as f:
            f.write(header + content)
            
        print(f"[OK] Ported {category}/{filename}")
        
    except subprocess.CalledProcessError as e:
        print(f"[FAIL] Failed to compile {category}/{filename}: {e}")

def main():
    ensure_dir(OUTPUT_DIR)
    
    # Categories from generate_bc.sh
    categories = [
        "basic_c_tests", 
        "basic_cpp_tests",
        "complex_tests",
        # Add more as needed
    ]
    
    for cat in categories:
        cat_path = os.path.join(TEST_SUITE_SRC, cat)
        if not os.path.exists(cat_path):
            continue
            
        print(f"Processing category: {cat}")
        for f in os.listdir(cat_path):
            if f.endswith(".c") or f.endswith(".cpp"):
                process_file(cat, f)

if __name__ == "__main__":
    main()
