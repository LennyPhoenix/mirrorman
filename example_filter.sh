#!/bin/bash

CMD=$1

if [ "$CMD" == "ext" ]; then
    EXT=$2
    if [ "$EXT" == "m4a" ] || [ "$EXT" == "wav" ] || [ "$EXT" == "ogg" ] || [ "$EXT" == "flac" ]; then
        echo "mp3"
        exit 0
    fi
    exit 1
fi

if [ "$CMD" == "run" ]; then
    IN=$2
    OUT=$3
    ffmpeg -nostdin -y -i "$IN" -vn -ar 44100 -ac 2 -b:a 192k "$OUT" 2> /dev/null
    exit $?
fi

echo "Usage: $0 <command>"
exit 1
