#!/bin/bash
# pre_deliver_lint.sh -- Run before presenting any file to the user.
# Catches the class of bugs that look fine but break downstream.
# Usage: bash pre_deliver_lint.sh <file1> [file2] ...

FAIL=0

for file in "$@"; do
    if [ ! -f "$file" ]; then
        echo "SKIP: $file (not found)"
        continue
    fi

    ext="${file##*.}"
    
    # Check 1: ASCII compliance for text files
    case "$ext" in
        md|toml|yaml|yml|skill|txt|cfg|ini|conf)
            if grep -Pn '[^\x00-\x7F]' "$file" > /dev/null 2>&1; then
                echo "FAIL: $file -- contains non-ASCII characters:"
                grep -Pn '[^\x00-\x7F]' "$file" | head -5
                FAIL=1
            else
                echo "OK:   $file -- ASCII clean"
            fi
            ;;
        rs)
            # Rust files may contain non-ASCII in strings -- skip ASCII check
            echo "OK:   $file -- Rust file (ASCII check skipped)"
            ;;
        *)
            echo "OK:   $file -- binary/other (no text checks)"
            ;;
    esac

    # Check 2: TOML validity for .toml files
    case "$ext" in
        toml)
            python3 -c "
import tomllib, sys
try:
    tomllib.loads(open('$file').read())
    print('OK:   $file -- TOML valid')
except Exception as e:
    print(f'FAIL: $file -- TOML invalid: {e}')
    sys.exit(1)
" || FAIL=1
            ;;
    esac

    # Check 3: Embedded TOML blocks in markdown
    case "$ext" in
        md)
            python3 -c "
import re, tomllib, sys
content = open('$file').read()
blocks = re.findall(r'\`\`\`toml\n(.*?)\`\`\`', content, re.DOTALL)
if blocks:
    for i, block in enumerate(blocks):
        try:
            tomllib.loads(block)
        except Exception as e:
            print(f'FAIL: $file -- embedded TOML block {i+1} invalid: {e}')
            sys.exit(1)
    print(f'OK:   $file -- {len(blocks)} embedded TOML block(s) valid')
" || FAIL=1
            ;;
    esac
done

if [ $FAIL -eq 0 ]; then
    echo ""
    echo "ALL CHECKS PASSED"
else
    echo ""
    echo "SOME CHECKS FAILED -- fix before delivering"
    exit 1
fi
