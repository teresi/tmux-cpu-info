# tmux-cpu-info

Displays a bar chart of CPU usage in tmux.

Displays in 1/8 increments, one per CPU: ` ▁▂▃▄▅▆▇█`


## Usage

Edit your `~/.tmux.conf`:
```bash
# install the plugin
set -g @plugin 'teresi/tmux-cpu-info'

# add the chart to your status, e.g.:
set-option -ag status-right '#($TMUX_PLUGIN_MANAGER_PATH/tmux-cpu-info/bars.py)'
```

## Example

![bars_sample.jpeg](screenshots/bars_sample.jpeg)


## Dependencies

Requires Tmux Plugin Manager, Python3, and the `psutil` module.

Tested on Linux.


## Other Plugins

This plugin was inspired by https://github.com/jdxcode/tmux-cpu-info

