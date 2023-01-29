#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
Formats CPU usage as a unicode bar chart.

Inspired by:
    https://github.com/jdxcode/tmux-cpu-info
"""

import psutil

# MAGIC there are 8 bars available and psutil gives percentage points [0..100]
DIV = 1/8*100

# MAGIC, see unicode 'Block Elements', https://en.wikipedia.org/wiki/Block_Elements
BARS = {0:" ", 1: "▁", 2: "▂", 3: "▃", 4: "▄", 5: "▅", 6: "▆", 7: "▇", 8: "█" }

def bar():
    """Return a string bar chart of CPU usage."""

    # NOTE cpu_percent blocks, how does that affect the caller, tmux?
    cpus = [int(u // DIV) for u in psutil.cpu_percent(interval=1, percpu=True)]
    cpus_bar = [BARS[u] for u in cpus]
    # INFO consider joining w/ a 'chr(0x200A)' i.e. HairSpace, to give some space around each bar
    # I found this space too large, and it appears to be the smallest space available
    bar = "".join(cpus_bar)
    return bar


if __name__ == '__main__':
    try:
        print(bar())
    except (KeyboardInterrupt, SystemExit):
        pass
