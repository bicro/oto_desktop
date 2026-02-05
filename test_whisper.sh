#!/bin/bash

# Test OpenRouter Whisper transcription API
# Usage: ./test_whisper.sh [audio_file.webm]

API_KEY="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"

# If no audio file provided, create a simple test
if [ -z "$1" ]; then
    echo "No audio file provided. Creating a test with a sample..."

    # Check if we have a test audio file
    if [ ! -f "/tmp/test_audio.webm" ]; then
        echo "Please provide an audio file: ./test_whisper.sh audio.webm"
        echo ""
        echo "To record a test file on Mac:"
        echo "  ffmpeg -f avfoundation -i ':0' -t 5 -c:a libopus /tmp/test_audio.webm"
        echo ""
        echo "Or use QuickTime to record and convert."
        exit 1
    fi
    AUDIO_FILE="/tmp/test_audio.webm"
else
    AUDIO_FILE="$1"
fi

echo "Testing with file: $AUDIO_FILE"
echo "File size: $(wc -c < "$AUDIO_FILE") bytes"
echo ""

# Test 1: Direct OpenRouter transcription endpoint
echo "=== Test 1: OpenRouter /audio/transcriptions ==="
curl -s -X POST "https://openrouter.ai/api/v1/audio/transcriptions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "HTTP-Referer: https://oto.frisson.app" \
  -H "X-Title: Oto Desktop" \
  -F "file=@$AUDIO_FILE" \
  -F "model=openai/whisper-large-v3" | jq .

echo ""
echo "=== Test 2: Try with whisper-1 model ==="
curl -s -X POST "https://openrouter.ai/api/v1/audio/transcriptions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "HTTP-Referer: https://oto.frisson.app" \
  -H "X-Title: Oto Desktop" \
  -F "file=@$AUDIO_FILE" \
  -F "model=openai/whisper-1" | jq .

echo ""
echo "=== Test 3: Check available models ==="
curl -s "https://openrouter.ai/api/v1/models" \
  -H "Authorization: Bearer $API_KEY" | jq '.data[] | select(.id | contains("whisper")) | {id, name}'
