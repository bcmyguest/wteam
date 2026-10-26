# Renders the landing-page GIF (scripted stand-in agents; see demo/setup.sh):
#   sh demo/render.sh
Output public/demo.gif

Require tmux

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 720
Set TypingSpeed 35ms
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set Padding 24
Set Margin 16
Set MarginFill "#1e1e2e"

Hide
Type "export PATH=/opt/wteam:$PATH WTEAM_FAKE_DIR=/tmp/wteam-demo/.wteam-fake WTEAM_DEMO_ROOT=/tmp/wteam-demo WTEAM_DEMO_PATH=/opt/wteam:$PATH && sh demo/setup.sh >/dev/null && cd /tmp/wteam-demo"
Enter
Type "tmux -f /dev/null -L wteamdemo new-session -- bash"
Enter
Type "sh tmux-start.sh"
Enter
Sleep 1s
Ctrl+L
Sleep 500ms
Show

Type "# greet CLI: ticket 01 kicks off the feature"
Enter
Sleep 500ms
Type "sed -n '1,6p' .wteam/demo/tickets/01-tracer-bullet.md"
Enter
Sleep 2.5s
Type "wteam --slug demo run --pairs 2"
Enter
Sleep 16s
Type "sed -n '1,12p' src/greet.py"
Enter
Sleep 2.5s
Type "git log --oneline -5"
Enter
Sleep 3.5s
