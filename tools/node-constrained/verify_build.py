"""Reuse the approved container builder without relabeling its source revision."""
import sys
from pathlib import Path
from support import module
HERE=Path(__file__).resolve().parent
PARENT=HERE.parent/'node-container'
original=sys.modules.get('support')
try:
    sys.modules['support']=module('approved_container_support',PARENT/'support.py')
    approved=module('approved_container_build_verifier',PARENT/'verify_build.py')
finally:
    if original is None:sys.modules.pop('support',None)
    else:sys.modules['support']=original
BINARIES=approved.BINARIES
verify=approved.verify
