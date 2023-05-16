#!/bin/sh
openocd -f openocd/olimex.cfg -f openocd/rpi4.cfg 2>/dev/null  &
sleep 2 &
gdb-multiarch  --command=resume.cfg
echo "Cleaning up..."
