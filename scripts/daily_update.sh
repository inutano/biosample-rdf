#!/bin/bash
set -euo pipefail

DATA_DIR="${DATA_DIR:-/data}"
INPUT_FILE="${DATA_DIR}/biosample_set.xml.gz"
OUTPUT_DIR="${DATA_DIR}/output"
NCBI_URL="https://ftp.ncbi.nlm.nih.gov/biosample/biosample_set.xml.gz"
CHUNK_SIZE="${CHUNK_SIZE:-100000}"

echo "=== BioSample RDF Daily Update ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Data dir: ${DATA_DIR}"

# Step 1: Download (skip if unchanged)
echo "--- Step 1: Download ---"
mkdir -p "${DATA_DIR}"
if [ -f "${INPUT_FILE}" ]; then
    echo "Checking for updates..."
    curl -z "${INPUT_FILE}" -o "${INPUT_FILE}" -fSL "${NCBI_URL}"
else
    echo "Downloading full dump..."
    curl -o "${INPUT_FILE}" -fSL "${NCBI_URL}"
fi
echo "Input file: $(ls -lh "${INPUT_FILE}" | awk '{print $5}')"

# Step 2: Convert
echo "--- Step 2: Convert ---"
rm -rf "${OUTPUT_DIR}"
biosample-rdf convert --input "${INPUT_FILE}" --output-dir "${OUTPUT_DIR}" --chunk-size "${CHUNK_SIZE}"

# Step 3: Validate
echo "--- Step 3: Validate ---"
biosample-rdf validate "${OUTPUT_DIR}"

# Step 4: Summary
echo "--- Complete ---"
cat "${OUTPUT_DIR}/manifest.json"

# Step 5: Notify (if webhook URL is set)
if [ -n "${NOTIFY_WEBHOOK:-}" ]; then
    RECORDS=$(jq -r '.records_processed' "${OUTPUT_DIR}/manifest.json")
    SKIPPED=$(jq -r '.records_skipped' "${OUTPUT_DIR}/manifest.json")
    curl -s -X POST "${NOTIFY_WEBHOOK}" \
        -H "Content-Type: application/json" \
        -d "{\"token\": \"${NOTIFY_TOKEN:-}\", \"title\": \"BioSample RDF\", \"message\": \"Done: ${RECORDS} records, ${SKIPPED} skipped\"}" \
        || true
fi
