import pathlib,sys
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
sys.path.append(str(HERE.parent/'management-check'))
import capture as baseline_capture
import verify as strict_json
helper=baseline_capture.helper
digest=baseline_capture.digest
require=strict_json.require
equal=strict_json.equal
load=strict_json.load
strict=strict_json.strict
read=strict_json.read
